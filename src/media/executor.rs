use crate::config::Config;
use crate::db::{AlchemistEvent, Decision, Job};
use crate::error::Result;
use crate::media::pipeline::{ExecutionStats, Executor, MediaMetadata};
use crate::orchestrator::Transcoder;
use crate::system::hardware::HardwareInfo;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct FfmpegExecutor {
    transcoder: Arc<Transcoder>,
    config: Arc<Config>,
    hw_info: Option<HardwareInfo>,
    event_tx: Arc<broadcast::Sender<AlchemistEvent>>,
    dry_run: bool,
}

impl FfmpegExecutor {
    pub fn new(
        transcoder: Arc<Transcoder>,
        config: Arc<Config>,
        hw_info: Option<HardwareInfo>,
        event_tx: Arc<broadcast::Sender<AlchemistEvent>>,
        dry_run: bool,
    ) -> Self {
        Self {
            transcoder,
            config,
            hw_info,
            event_tx,
            dry_run,
        }
    }
}

#[async_trait]
impl Executor for FfmpegExecutor {
    async fn execute(
        &self,
        job: &Job,
        _decision: &Decision,
        metadata: &MediaMetadata,
    ) -> Result<ExecutionStats> {
        let input_path = PathBuf::from(&job.input_path);
        // Output path logic? Ideally passed in or determined by Agent.
        // For now, let's assume we derive it or it's passed.
        // The trait doesn't specify output path in execute(), but Plan suggests `execute(decision, input, output)`.
        // Let's check trait signature in pipeline.rs
        // pipeline.rs: async fn execute(&self, job: &Job, decision: &Decision) -> Result<ExecutionStats>;

        // So we must determine output path here.
        let output_path = PathBuf::from(&job.output_path); // Use job's output path

        self.transcoder
            .transcode_media(
                &input_path,
                &output_path,
                self.hw_info.as_ref(),
                self.config.transcode.quality_profile,
                self.config.hardware.cpu_preset,
                self.config.transcode.output_codec,
                self.config.transcode.threads,
                self.config.transcode.allow_fallback,
                self.config.transcode.hdr_mode,
                self.config.transcode.tonemap_algorithm,
                self.config.transcode.tonemap_peak,
                self.config.transcode.tonemap_desat,
                self.dry_run,
                metadata,
                Some((job.id, self.event_tx.clone())),
            )
            .await?;

        // TODO: Populate actual stats from somewhere?
        // Transcoder doesn't return detailed stats yet, it just returns Ok(()).
        // Agent::finalize_job calculates stats from file system.
        // For now, return empty stats or partial.

        // We need metadata for duration... Transcoder needs it.
        // But execute() doesn't receive metadata.
        // Transcoder needs `FfprobeMetadata`.
        // This suggests Executor trait needs Metadata or Job should store it.
        // Currently Job doesn't store full metadata.

        // Workaround: We might have to re-probe or assume duration is known?
        // Transcoder uses metadata.format.duration.

        // Alternative: Update Executor trait to take Metadata.
        // pipeline.rs check:
        // pub trait Executor: Send + Sync {
        //     async fn execute(&self, job: &Job, decision: &Decision) -> Result<ExecutionStats>;
        // }

        // I should update the Executor trait to include MediaMetadata.
        // Or re-probe. Re-probing is wasteful.
        // Let's update the trait.

        // For now, I'll write the file but leave a TODO or construct dummy metadata if I can't change trait easily.
        // But I CAN change trait easily.

        Ok(ExecutionStats {
            encode_time_secs: 0.0, // Agent calculates this
            input_size: 0,
            output_size: 0,
            vmaf: None,
        })
    }
}
