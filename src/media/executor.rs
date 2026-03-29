use crate::db::{AlchemistEvent, Db, EventChannels, Job, JobEvent};
use crate::error::Result;
use crate::media::pipeline::{
    Encoder, ExecutionResult, ExecutionStats, Executor, MediaAnalysis, TranscodePlan,
};
use crate::orchestrator::{
    AsyncExecutionObserver, ExecutionObserver, TranscodeRequest, Transcoder,
};
use crate::system::hardware::HardwareInfo;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, broadcast};

pub struct FfmpegExecutor {
    transcoder: Arc<Transcoder>,
    db: Arc<Db>,
    hw_info: Option<HardwareInfo>,
    event_tx: Arc<broadcast::Sender<AlchemistEvent>>,
    event_channels: Arc<EventChannels>,
    dry_run: bool,
}

impl FfmpegExecutor {
    pub fn new(
        transcoder: Arc<Transcoder>,
        db: Arc<Db>,
        hw_info: Option<HardwareInfo>,
        event_tx: Arc<broadcast::Sender<AlchemistEvent>>,
        event_channels: Arc<EventChannels>,
        dry_run: bool,
    ) -> Self {
        Self {
            transcoder,
            db,
            hw_info,
            event_tx,
            event_channels,
            dry_run,
        }
    }
}

struct JobExecutionObserver {
    job_id: i64,
    db: Arc<Db>,
    event_tx: Arc<broadcast::Sender<AlchemistEvent>>,
    event_channels: Arc<EventChannels>,
    last_progress: Mutex<Option<(f64, Instant)>>,
}

impl JobExecutionObserver {
    fn new(
        job_id: i64,
        db: Arc<Db>,
        event_tx: Arc<broadcast::Sender<AlchemistEvent>>,
        event_channels: Arc<EventChannels>,
    ) -> Self {
        Self {
            job_id,
            db,
            event_tx,
            event_channels,
            last_progress: Mutex::new(None),
        }
    }
}

impl AsyncExecutionObserver for JobExecutionObserver {
    async fn on_log(&self, message: String) {
        // Send to typed channel
        let _ = self.event_channels.jobs.send(JobEvent::Log {
            level: "info".to_string(),
            job_id: Some(self.job_id),
            message: message.clone(),
        });
        // Also send to legacy channel for backwards compatibility
        let _ = self.event_tx.send(AlchemistEvent::Log {
            level: "info".to_string(),
            job_id: Some(self.job_id),
            message: message.clone(),
        });
        if let Err(err) = self.db.add_log("info", Some(self.job_id), &message).await {
            tracing::warn!(
                "Failed to persist transcode log for job {}: {}",
                self.job_id,
                err
            );
        }
    }

    async fn on_progress(
        &self,
        progress: crate::media::ffmpeg::FFmpegProgress,
        total_duration: f64,
    ) {
        let percentage = progress.percentage(total_duration).clamp(0.0, 100.0);
        let now = Instant::now();
        let mut last_progress = self.last_progress.lock().await;
        let should_persist = match *last_progress {
            Some((last_pct, last_time)) => {
                percentage >= last_pct + 0.5
                    || now.duration_since(last_time) >= Duration::from_secs(2)
            }
            None => true,
        };

        if should_persist {
            if let Err(err) = self.db.update_job_progress(self.job_id, percentage).await {
                tracing::warn!(
                    "Failed to persist progress for job {}: {}",
                    self.job_id,
                    err
                );
            } else {
                *last_progress = Some((percentage, now));
            }
        }

        // Send to typed channel
        let _ = self.event_channels.jobs.send(JobEvent::Progress {
            job_id: self.job_id,
            percentage,
            time: progress.time.clone(),
        });
        // Also send to legacy channel for backwards compatibility
        let _ = self.event_tx.send(AlchemistEvent::Progress {
            job_id: self.job_id,
            percentage,
            time: progress.time,
        });
    }
}

impl Executor for FfmpegExecutor {
    async fn execute(
        &self,
        job: &Job,
        plan: &TranscodePlan,
        analysis: &MediaAnalysis,
    ) -> Result<ExecutionResult> {
        let input_path = PathBuf::from(&job.input_path);
        let output_path = plan
            .output_path
            .as_ref()
            .cloned()
            .unwrap_or_else(|| PathBuf::from(&job.output_path));
        let encoder = plan.encoder;
        let planned_output_codec = plan.output_codec.unwrap_or_else(|| {
            encoder
                .map(Encoder::output_codec)
                .unwrap_or(plan.requested_codec)
        });
        let used_backend = plan.backend.or_else(|| encoder.map(Encoder::backend));
        let observer: Arc<dyn ExecutionObserver> = Arc::new(JobExecutionObserver::new(
            job.id,
            self.db.clone(),
            self.event_tx.clone(),
            self.event_channels.clone(),
        ));

        self.transcoder
            .transcode_media(TranscodeRequest {
                job_id: Some(job.id),
                input: &input_path,
                output: &output_path,
                hw_info: self.hw_info.as_ref(),
                dry_run: self.dry_run,
                metadata: &analysis.metadata,
                plan,
                observer: Some(observer.clone()),
            })
            .await?;

        if matches!(
            plan.subtitles,
            crate::media::pipeline::SubtitleStreamPlan::Extract { .. }
        ) {
            self.transcoder
                .extract_subtitles(TranscodeRequest {
                    job_id: Some(job.id),
                    input: &input_path,
                    output: &output_path,
                    hw_info: self.hw_info.as_ref(),
                    dry_run: self.dry_run,
                    metadata: &analysis.metadata,
                    plan,
                    observer: Some(observer),
                })
                .await?;
        }

        let actual_probe = if !self.dry_run && output_path.exists() {
            crate::media::analyzer::Analyzer::probe_output_details(&output_path)
                .await
                .ok()
        } else {
            None
        };
        let actual_output_codec = actual_probe
            .as_ref()
            .and_then(|probe| output_codec_from_name(&probe.codec_name));
        let actual_encoder_name = actual_probe
            .as_ref()
            .and_then(|probe| {
                probe
                    .stream_encoder_tag
                    .clone()
                    .or_else(|| probe.format_encoder_tag.clone())
            })
            .or_else(|| {
                if plan.is_remux {
                    Some("copy".to_string())
                } else {
                    encoder.map(|encoder| encoder.ffmpeg_encoder_name().to_string())
                }
            });
        let codec_mismatch = actual_output_codec
            .is_some_and(|actual_output_codec| actual_output_codec != planned_output_codec);
        let encoder_mismatch = encoder.is_some_and(|encoder| {
            actual_probe
                .as_ref()
                .and_then(|probe| probe.stream_encoder_tag.as_deref())
                .is_some_and(|tag| !encoder_tag_matches(encoder, tag))
        });

        if let (true, Some(codec)) = (codec_mismatch, actual_output_codec) {
            tracing::warn!(
                "Job {}: Planned codec {} but output probed as {}",
                job.id,
                planned_output_codec.as_str(),
                codec.as_str()
            );
        }

        if let (true, Some(enc)) = (encoder_mismatch, encoder) {
            tracing::warn!(
                "Job {}: Planned encoder {} but stream tag reported {:?}",
                job.id,
                enc.ffmpeg_encoder_name(),
                actual_probe
                    .as_ref()
                    .and_then(|probe| probe.stream_encoder_tag.as_deref())
            );
        }

        Ok(ExecutionResult {
            requested_codec: plan.requested_codec,
            planned_output_codec,
            requested_encoder: encoder,
            used_encoder: encoder,
            used_backend,
            fallback: plan.fallback.clone(),
            fallback_occurred: plan.fallback.is_some() || codec_mismatch || encoder_mismatch,
            actual_output_codec,
            actual_encoder_name,
            stats: ExecutionStats {
                encode_time_secs: 0.0,
                input_size: 0,
                output_size: 0,
                vmaf: None,
            },
        })
    }
}

fn output_codec_from_name(codec: &str) -> Option<crate::config::OutputCodec> {
    if codec.eq_ignore_ascii_case("av1") {
        Some(crate::config::OutputCodec::Av1)
    } else if codec.eq_ignore_ascii_case("hevc") || codec.eq_ignore_ascii_case("h265") {
        Some(crate::config::OutputCodec::Hevc)
    } else if codec.eq_ignore_ascii_case("h264") || codec.eq_ignore_ascii_case("avc") {
        Some(crate::config::OutputCodec::H264)
    } else {
        None
    }
}

fn encoder_tag_matches(requested: crate::media::pipeline::Encoder, encoder_tag: &str) -> bool {
    let tag = encoder_tag.to_ascii_lowercase();
    let expected_markers: &[&str] = match requested {
        crate::media::pipeline::Encoder::Av1Qsv
        | crate::media::pipeline::Encoder::HevcQsv
        | crate::media::pipeline::Encoder::H264Qsv => &["qsv"],
        crate::media::pipeline::Encoder::Av1Nvenc
        | crate::media::pipeline::Encoder::HevcNvenc
        | crate::media::pipeline::Encoder::H264Nvenc => &["nvenc"],
        crate::media::pipeline::Encoder::Av1Vaapi
        | crate::media::pipeline::Encoder::HevcVaapi
        | crate::media::pipeline::Encoder::H264Vaapi => &["vaapi"],
        crate::media::pipeline::Encoder::Av1Videotoolbox
        | crate::media::pipeline::Encoder::HevcVideotoolbox
        | crate::media::pipeline::Encoder::H264Videotoolbox => &["videotoolbox"],
        crate::media::pipeline::Encoder::Av1Amf
        | crate::media::pipeline::Encoder::HevcAmf
        | crate::media::pipeline::Encoder::H264Amf => &["amf"],
        crate::media::pipeline::Encoder::Av1Svt => &["svtav1", "svt-av1", "libsvtav1"],
        crate::media::pipeline::Encoder::Av1Aom => &["libaom", "aom"],
        crate::media::pipeline::Encoder::HevcX265 => &["x265", "libx265"],
        crate::media::pipeline::Encoder::H264X264 => &["x264", "libx264"],
    };

    expected_markers.iter().any(|marker| tag.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::media::pipeline::Encoder;
    use crate::orchestrator::LocalExecutionObserver;
    use std::path::Path;
    use std::sync::Arc;
    use std::time::SystemTime;
    use tokio::sync::broadcast;

    #[test]
    fn output_codec_mapping_handles_common_aliases() {
        assert_eq!(
            output_codec_from_name("av1"),
            Some(crate::config::OutputCodec::Av1)
        );
        assert_eq!(
            output_codec_from_name("hevc"),
            Some(crate::config::OutputCodec::Hevc)
        );
        assert_eq!(
            output_codec_from_name("h265"),
            Some(crate::config::OutputCodec::Hevc)
        );
        assert_eq!(
            output_codec_from_name("h264"),
            Some(crate::config::OutputCodec::H264)
        );
        assert_eq!(
            output_codec_from_name("avc"),
            Some(crate::config::OutputCodec::H264)
        );
        assert_eq!(output_codec_from_name("vp9"), None);
    }

    #[test]
    fn encoder_tag_matching_uses_stream_encoder_markers() {
        assert!(encoder_tag_matches(
            Encoder::Av1Nvenc,
            "Lavc61.3.100 av1_nvenc"
        ));
        assert!(!encoder_tag_matches(
            Encoder::Av1Nvenc,
            "Lavc61.3.100 libsvtav1"
        ));
    }

    fn temp_db_path(prefix: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("{prefix}_{}.db", rand::random::<u64>()));
        path
    }

    #[tokio::test]
    async fn job_execution_observer_persists_logs_and_progress()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let db_path = temp_db_path("alchemist_observer");
        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let _ = db
            .enqueue_job(
                Path::new("input.mkv"),
                Path::new("output.mkv"),
                SystemTime::UNIX_EPOCH,
            )
            .await?;
        let job = db.get_job_by_input_path("input.mkv").await?.expect("job");
        let (tx, mut rx) = broadcast::channel(8);
        let (jobs_tx, _) = broadcast::channel(100);
        let (config_tx, _) = broadcast::channel(10);
        let (system_tx, _) = broadcast::channel(10);
        let event_channels = Arc::new(crate::db::EventChannels {
            jobs: jobs_tx,
            config: config_tx,
            system: system_tx,
        });
        let observer = JobExecutionObserver::new(job.id, db.clone(), Arc::new(tx), event_channels);

        LocalExecutionObserver::on_log(&observer, "ffmpeg line".to_string()).await;
        LocalExecutionObserver::on_progress(
            &observer,
            crate::media::ffmpeg::FFmpegProgress {
                time: "00:00:02.00".to_string(),
                time_seconds: 2.0,
                ..Default::default()
            },
            10.0,
        )
        .await;

        let logs = db.get_logs(10, 0).await?;
        assert_eq!(logs[0].message, "ffmpeg line");

        let updated = db.get_job(job.id).await?.expect("updated");
        assert!((updated.progress - 20.0).abs() < 0.01);

        let first = rx.recv().await?;
        assert!(matches!(first, AlchemistEvent::Log { .. }));
        let second = rx.recv().await?;
        assert!(matches!(second, AlchemistEvent::Progress { .. }));

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
