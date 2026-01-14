use crate::config::{CpuPreset, QualityProfile};
use crate::error::{AlchemistError, Result};
use crate::media::ffmpeg::{FFmpegCommandBuilder, FFmpegProgress};
use crate::media::pipeline::{Encoder, RateControl};
use crate::system::hardware::HardwareInfo;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{broadcast, oneshot};
use tracing::{error, info, warn};

pub struct Transcoder {
    cancel_channels: Arc<Mutex<HashMap<i64, oneshot::Sender<()>>>>,
}

pub struct TranscodeRequest<'a> {
    pub input: &'a Path,
    pub output: &'a Path,
    pub hw_info: Option<&'a HardwareInfo>,
    pub quality_profile: QualityProfile,
    pub cpu_preset: CpuPreset,
    pub threads: usize,
    pub allow_fallback: bool,
    pub hdr_mode: crate::config::HdrMode,
    pub tonemap_algorithm: crate::config::TonemapAlgorithm,
    pub tonemap_peak: f32,
    pub tonemap_desat: f32,
    pub dry_run: bool,
    pub metadata: &'a crate::media::pipeline::MediaMetadata,
    pub encoder: Option<Encoder>,
    pub rate_control: Option<RateControl>,
    pub event_target: Option<(i64, Arc<broadcast::Sender<crate::db::AlchemistEvent>>)>,
}

impl Default for Transcoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Transcoder {
    pub fn new() -> Self {
        Self {
            cancel_channels: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn cancel_job(&self, job_id: i64) -> bool {
        let mut channels = match self.cancel_channels.lock() {
            Ok(channels) => channels,
            Err(e) => {
                error!("Cancel channels lock poisoned: {}", e);
                return false;
            }
        };
        match channels.remove(&job_id) {
            Some(tx) => {
                let _ = tx.send(());
                true
            }
            None => false,
        }
    }

    pub async fn transcode_media(&self, request: TranscodeRequest<'_>) -> Result<()> {
        if request.dry_run {
            info!(
                "[DRY RUN] Transcoding {:?} to {:?}",
                request.input, request.output
            );
            return Ok(());
        }

        if request.input == request.output {
            return Err(AlchemistError::Config(
                "Output path matches input path".into(),
            ));
        }

        // Ensure output directory exists
        if let Some(parent) = request.output.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    error!("Failed to create output directory {:?}: {}", parent, e);
                    AlchemistError::FFmpeg(format!(
                        "Failed to create output directory {:?}: {}",
                        parent, e
                    ))
                })?;
            }
        }

        let mut cmd = FFmpegCommandBuilder::new(request.input, request.output)
            .with_hardware(request.hw_info)
            .with_profile(request.quality_profile)
            .with_cpu_preset(request.cpu_preset)
            .with_threads(request.threads)
            .with_allow_fallback(request.allow_fallback)
            .with_encoder(request.encoder)
            .with_rate_control(request.rate_control)
            .with_hdr_settings(
                request.hdr_mode,
                request.tonemap_algorithm,
                request.tonemap_peak,
                request.tonemap_desat,
                Some(request.metadata),
            )
            .build();

        info!("Executing FFmpeg command: {:?}", cmd);

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let total_duration = request.metadata.duration_secs;

        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("Starting transcode:");
        info!("  Input:  {:?}", request.input);
        info!("  Output: {:?}", request.output);
        info!("  Duration: {:.2}s", total_duration);
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let mut child = cmd
            .spawn()
            .map_err(|e| AlchemistError::FFmpeg(format!("Failed to spawn FFmpeg: {}", e)))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AlchemistError::FFmpeg("Failed to capture stderr".into()))?;

        let (kill_tx, kill_rx) = oneshot::channel();
        let job_id = request.event_target.as_ref().map(|(id, _)| *id);

        if let Some(id) = job_id {
            match self.cancel_channels.lock() {
                Ok(mut channels) => {
                    channels.insert(id, kill_tx);
                }
                Err(e) => {
                    error!("Cancel channels lock poisoned: {}", e);
                }
            }
        }

        let mut reader = BufReader::new(stderr).lines();
        let event_target_clone = request.event_target.clone();

        let mut kill_rx = kill_rx;
        let mut killed = false;
        let mut last_lines = std::collections::VecDeque::with_capacity(10);

        loop {
            tokio::select! {
                line_res = reader.next_line() => {
                    match line_res {
                        Ok(Some(line)) => {
                            last_lines.push_back(line.clone());
                            if last_lines.len() > 10 {
                                last_lines.pop_front();
                            }

                            if let Some((job_id, ref tx)) = event_target_clone {
                                let _ = tx.send(crate::db::AlchemistEvent::Log {
                                    level: "info".to_string(),
                                    job_id: Some(job_id),
                                    message: line.clone()
                                });

                                if let Some(progress) = FFmpegProgress::parse_line(&line) {
                                    let percentage: f64 = progress.percentage(total_duration);
                                    let _ = tx.send(crate::db::AlchemistEvent::Progress {
                                        job_id,
                                        percentage,
                                        time: progress.time
                                    });
                                }
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            error!("Error reading FFmpeg stderr: {}", e);
                            break;
                        }
                    }
                }
                _ = &mut kill_rx => {
                    warn!("Job {:?} cancelled. Killing FFmpeg process...", job_id);
                    let _ = child.kill().await;
                    killed = true;
                    if let Some(id) = job_id {
                        if let Ok(mut channels) = self.cancel_channels.lock() {
                            channels.remove(&id);
                        } else {
                            error!("Cancel channels lock poisoned while removing job: {}", id);
                        }
                    }
                    break;
                }
            }
        }

        let status: std::process::ExitStatus = child.wait().await?;

        if let Some(id) = job_id {
            if let Ok(mut channels) = self.cancel_channels.lock() {
                channels.remove(&id);
            } else {
                error!("Cancel channels lock poisoned while removing job: {}", id);
            }
        }

        if killed {
            return Err(AlchemistError::Cancelled);
        }

        if status.success() {
            info!("Transcode successful: {:?}", request.output);
            Ok(())
        } else {
            let error_detail = last_lines.make_contiguous().join("\n");
            error!(
                "FFmpeg failed with status: {}\nDetails:\n{}",
                status, error_detail
            );
            Err(AlchemistError::FFmpeg(format!(
                "FFmpeg failed ({}). Last output:\n{}",
                status, error_detail
            )))
        }
    }

    // Redundant parse_duration removed, use FFmpegProgress instead.
}
