use crate::db::Job;
use crate::error::Result;
use crate::media::analyzer::FfmpegAnalyzer;
use crate::media::executor::FfmpegExecutor;
use crate::media::planner::{build_hardware_capabilities, BasicPlanner};
use crate::orchestrator::Transcoder;
use crate::system::hardware::HardwareInfo;
use crate::telemetry::{encoder_label, hardware_label, resolution_bucket, TelemetryEvent};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaMetadata {
    pub path: PathBuf,
    pub duration_secs: f64,
    pub codec_name: String,
    pub width: u32,
    pub height: u32,
    pub bit_depth: Option<u8>,
    pub color_primaries: Option<String>,
    pub color_transfer: Option<String>,
    pub color_space: Option<String>,
    pub color_range: Option<String>,
    pub size_bytes: u64,
    pub video_bitrate_bps: Option<u64>,
    pub container_bitrate_bps: Option<u64>,
    pub fps: f64,
    pub container: String,
    pub audio_codec: Option<String>,
    pub audio_channels: Option<u32>,
    pub dynamic_range: DynamicRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredMedia {
    pub path: PathBuf,
    pub mtime: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisWarning {
    MissingVideoBitrate,
    MissingContainerBitrate,
    MissingDuration,
    MissingFps,
    MissingBitDepth,
    UnrecognizedPixelFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAnalysis {
    pub metadata: MediaMetadata,
    pub warnings: Vec<AnalysisWarning>,
    pub confidence: AnalysisConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub encode_time_secs: f64,
    pub input_size: u64,
    pub output_size: u64,
    pub vmaf: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRange {
    Sdr,
    Hdr10,
    Hlg,
    DolbyVision,
    Unknown,
}

impl DynamicRange {
    pub fn is_hdr(&self) -> bool {
        matches!(self, DynamicRange::Hdr10 | DynamicRange::Hlg | DynamicRange::DolbyVision)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranscodeDecision {
    Skip { reason: String },
    Transcode { reason: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Encoder {
    Av1Qsv,
    Av1Nvenc,
    Av1Vaapi,
    Av1Videotoolbox,
    Av1Amf,
    Av1Svt,
    Av1Aom,
    HevcQsv,
    HevcNvenc,
    HevcVaapi,
    HevcVideotoolbox,
    HevcAmf,
    HevcX265,
    H264Qsv,
    H264Nvenc,
    H264Vaapi,
    H264Videotoolbox,
    H264Amf,
    H264X264,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderLimits {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub max_bitrate_bps: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareCapabilities {
    pub encoders: Vec<Encoder>,
    pub constraints: HashMap<Encoder, EncoderLimits>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateControl {
    Crf { value: u8 },
    Cq { value: u8 },
    QsvQuality { value: u8 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub decision: TranscodeDecision,
    pub output_path: Option<PathBuf>,
    pub target_container: Option<String>,
    pub encoder: Option<Encoder>,
    pub rate_control: Option<RateControl>,
    pub allow_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub used_encoder: Encoder,
    pub fallback_occurred: bool,
    pub stats: ExecutionStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobFailure {
    Transient,
    MediaCorrupt,
    EncoderUnavailable,
    PlannerBug,
}

#[async_trait]
pub trait Analyzer: Send + Sync {
    async fn analyze(&self, path: &Path) -> Result<MediaAnalysis>;
}

#[async_trait]
pub trait Planner: Send + Sync {
    async fn plan(
        &self,
        analysis: &MediaAnalysis,
        hardware: &HardwareCapabilities,
        output_extension: &str,
    ) -> Result<ExecutionPlan>;
}

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute(
        &self,
        job: &Job,
        plan: &ExecutionPlan,
        analysis: &MediaAnalysis,
    ) -> Result<ExecutionResult>;
}

pub struct Pipeline {
    db: Arc<crate::db::Db>,
    orchestrator: Arc<Transcoder>,
    config: Arc<RwLock<crate::config::Config>>,
    hw_info: Arc<Option<HardwareInfo>>,
    tx: Arc<broadcast::Sender<crate::db::AlchemistEvent>>,
    dry_run: bool,
}

impl Pipeline {
    pub fn new(
        db: Arc<crate::db::Db>,
        orchestrator: Arc<Transcoder>,
        config: Arc<RwLock<crate::config::Config>>,
        hw_info: Arc<Option<HardwareInfo>>,
        tx: Arc<broadcast::Sender<crate::db::AlchemistEvent>>,
        dry_run: bool,
    ) -> Self {
        Self {
            db,
            orchestrator,
            config,
            hw_info,
            tx,
            dry_run,
        }
    }

    pub async fn enqueue_discovered(&self, discovered: DiscoveredMedia) -> Result<()> {
        enqueue_discovered_with_db(&self.db, discovered).await
    }
}

pub async fn enqueue_discovered_with_db(
    db: &crate::db::Db,
    discovered: DiscoveredMedia,
) -> Result<()> {
    let settings = match db.get_file_settings().await {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to fetch file settings, using defaults: {}", e);
            crate::db::FileSettings {
                id: 1,
                delete_source: false,
                output_extension: "mkv".to_string(),
                output_suffix: "-alchemist".to_string(),
                replace_strategy: "keep".to_string(),
            }
        }
    };

    let output_path = settings.output_path_for(&discovered.path);
    if output_path.exists() && !settings.should_replace_existing_output() {
        tracing::info!(
            "Skipping {:?} (output exists, replace_strategy = keep)",
            discovered.path
        );
        return Ok(());
    }

    db.enqueue_job(&discovered.path, &output_path, discovered.mtime)
        .await
}

impl Pipeline {
    pub async fn process_job(&self, job: Job) -> std::result::Result<(), JobFailure> {
        let file_path = PathBuf::from(&job.input_path);

        let file_settings = match self.db.get_file_settings().await {
            Ok(settings) => settings,
            Err(e) => {
                tracing::error!("Failed to fetch file settings, using defaults: {}", e);
                crate::db::FileSettings {
                    id: 1,
                    delete_source: false,
                    output_extension: "mkv".to_string(),
                    output_suffix: "-alchemist".to_string(),
                    replace_strategy: "keep".to_string(),
                }
            }
        };

        let mut output_path = file_settings.output_path_for(&file_path);

        if file_path == output_path {
            tracing::error!(
                "Job {}: Output path matches input path; refusing to overwrite source.",
                job.id
            );
            let _ = self
                .db
                .add_decision(job.id, "skip", "Output path matches input path")
                .await;
            let _ = self.update_job_state(job.id, crate::db::JobState::Skipped).await;
            return Ok(());
        }

        if output_path.exists() && !file_settings.should_replace_existing_output() {
            tracing::info!(
                "Job {}: Output exists and replace_strategy is keep. Skipping.",
                job.id
            );
            let _ = self
                .db
                .add_decision(job.id, "skip", "Output already exists")
                .await;
            let _ = self.update_job_state(job.id, crate::db::JobState::Skipped).await;
            return Ok(());
        }

        let file_name = file_path.file_name().unwrap_or_default();
        tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        tracing::info!("📹 Processing Job #{}: {:?}", job.id, file_name);
        tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let start_time = std::time::Instant::now();

        tracing::info!("[Job {}] Phase 1: Analyzing media...", job.id);
        if self.db.increment_attempt_count(job.id).await.is_err() {
            return Err(JobFailure::Transient);
        }
        if self
            .update_job_state(job.id, crate::db::JobState::Analyzing)
            .await
            .is_err()
        {
            return Err(JobFailure::Transient);
        }
        self.update_job_progress(job.id, 0.0).await;

        let analyzer = FfmpegAnalyzer;
        let analyze_start = std::time::Instant::now();
        let analysis = match analyzer.analyze(&file_path).await {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("Job {}: Probing failed: {}", job.id, e);
                let _ = self.update_job_state(job.id, crate::db::JobState::Failed).await;
                return Err(JobFailure::MediaCorrupt);
            }
        };
        let metadata = &analysis.metadata;

        let analyze_duration = analyze_start.elapsed();
        tracing::info!(
            "[Job {}] Analysis complete in {:.2}s",
            job.id,
            analyze_duration.as_secs_f64()
        );

        tracing::info!(
            "[Job {}] Resolution: {}x{}",
            job.id,
            metadata.width,
            metadata.height
        );
        tracing::info!("[Job {}] Codec: {}", job.id, metadata.codec_name);

        let config_snapshot = self.config.read().await.clone();
        let encoder_caps = Arc::new(crate::media::ffmpeg::encoder_caps_clone());
        let planner = BasicPlanner::new(
            Arc::new(config_snapshot.clone()),
            self.hw_info.as_ref().clone(),
            encoder_caps.clone(),
        );
        let hardware_caps =
            build_hardware_capabilities(&encoder_caps, self.hw_info.as_ref().as_ref());
        let mut plan = match planner
            .plan(&analysis, &hardware_caps, &file_settings.output_extension)
            .await
        {
            Ok(plan) => plan,
            Err(e) => {
                tracing::error!("Job {}: Planner failed: {}", job.id, e);
                let _ = self.update_job_state(job.id, crate::db::JobState::Failed).await;
                return Err(JobFailure::PlannerBug);
            }
        };

        if matches!(plan.decision, TranscodeDecision::Transcode { .. }) {
            output_path = file_settings.output_path_for(&file_path);
            plan.output_path = Some(output_path.clone());
        }

        let (should_encode, reason) = match &plan.decision {
            TranscodeDecision::Transcode { reason } => (true, reason.clone()),
            TranscodeDecision::Skip { reason } => (false, reason.clone()),
        };

        if !should_encode {
            tracing::info!("Decision: SKIP Job {} - {}", job.id, &reason);
            let _ = self.db.add_decision(job.id, "skip", &reason).await;
            let _ = self.update_job_state(job.id, crate::db::JobState::Skipped).await;
            return Ok(());
        }

        tracing::info!("Decision: ENCODE Job {} - {}", job.id, &reason);
        let _ = self.db.add_decision(job.id, "encode", &reason).await;
        let _ = self.tx.send(crate::db::AlchemistEvent::Decision {
            job_id: job.id,
            action: "encode".to_string(),
            reason: reason.clone(),
        });

        if self
            .update_job_state(job.id, crate::db::JobState::Encoding)
            .await
            .is_err()
        {
            return Err(JobFailure::Transient);
        }
        self.update_job_progress(job.id, 0.0).await;

        self.emit_telemetry_event(TelemetryEventParams {
            telemetry_enabled: config_snapshot.system.enable_telemetry,
            output_codec: config_snapshot.transcode.output_codec,
            metadata,
            event_type: "job_started",
            status: None,
            failure_reason: None,
            input_size_bytes: Some(metadata.size_bytes),
            output_size_bytes: None,
            duration_ms: None,
            speed_factor: None,
        })
        .await;

        let executor = FfmpegExecutor::new(
            self.orchestrator.clone(),
            Arc::new(config_snapshot.clone()),
            self.hw_info.as_ref().clone(),
            self.tx.clone(),
            self.dry_run,
        );

        match executor.execute(&job, &plan, &analysis).await {
            Ok(result) => {
                if result.fallback_occurred && !plan.allow_fallback {
                    tracing::error!("Job {}: Encoder fallback detected and not allowed.", job.id);
                    let _ = self.update_job_state(job.id, crate::db::JobState::Failed).await;
                    return Err(JobFailure::EncoderUnavailable);
                }

                self.finalize_job(job, &file_path, &output_path, start_time, metadata)
                    .await
                    .map_err(|_| JobFailure::Transient)
            }
            Err(e) => {
                if output_path.exists() {
                    if let Err(err) = tokio::fs::remove_file(&output_path).await {
                        tracing::warn!(
                            "Job {}: Failed to remove partial output {:?}: {}",
                            job.id,
                            output_path,
                            err
                        );
                    } else {
                        tracing::info!("Job {}: Removed partial output {:?}", job.id, output_path);
                    }
                }
                let failure_reason = if let crate::error::AlchemistError::Cancelled = e {
                    "cancelled"
                } else {
                    "transcode_failed"
                };
                self.emit_telemetry_event(TelemetryEventParams {
                    telemetry_enabled: config_snapshot.system.enable_telemetry,
                    output_codec: config_snapshot.transcode.output_codec,
                    metadata,
                    event_type: "job_finished",
                    status: Some("failure"),
                    failure_reason: Some(failure_reason),
                    input_size_bytes: Some(metadata.size_bytes),
                    output_size_bytes: None,
                    duration_ms: Some(start_time.elapsed().as_millis() as u64),
                    speed_factor: None,
                })
                .await;

                if let crate::error::AlchemistError::Cancelled = e {
                    let _ = self.update_job_state(job.id, crate::db::JobState::Cancelled).await;
                } else {
                    tracing::error!("Job {}: Transcode failed: {}", job.id, e);
                    let _ = self.update_job_state(job.id, crate::db::JobState::Failed).await;
                }
                Err(map_failure(&e))
            }
        }
    }

    async fn update_job_state(
        &self,
        job_id: i64,
        status: crate::db::JobState,
    ) -> Result<()> {
        if let Err(e) = self.db.update_job_status(job_id, status).await {
            tracing::error!("Failed to update job {} status {:?}: {}", job_id, status, e);
            return Err(e);
        }
        let _ = self.tx.send(crate::db::AlchemistEvent::JobStateChanged { job_id, status });
        Ok(())
    }

    async fn update_job_progress(&self, job_id: i64, progress: f64) {
        if let Err(e) = self.db.update_job_progress(job_id, progress).await {
            tracing::error!("Failed to update job progress: {}", e);
        }
    }

    async fn finalize_job(
        &self,
        job: Job,
        input_path: &Path,
        output_path: &Path,
        start_time: std::time::Instant,
        metadata: &MediaMetadata,
    ) -> Result<()> {
        let job_id = job.id;
        let input_metadata = std::fs::metadata(input_path)?;
        let input_size = input_metadata.len();

        let output_metadata = std::fs::metadata(output_path)?;
        let output_size = output_metadata.len();

        if input_size == 0 {
            tracing::error!("Job {}: Input file is empty. Finalizing as failed.", job_id);
            self.update_job_state(job_id, crate::db::JobState::Failed)
                .await?;
            return Ok(());
        }

        let reduction = 1.0 - (output_size as f64 / input_size as f64);
        let encode_duration = start_time.elapsed().as_secs_f64();

        let config = self.config.read().await;
        let telemetry_enabled = config.system.enable_telemetry;
        let output_codec = config.transcode.output_codec;

        if output_size == 0 || reduction < config.transcode.size_reduction_threshold {
            tracing::warn!(
                "Job {}: Size reduction gate failed ({:.2}%). Reverting.",
                job_id,
                reduction * 100.0
            );
            let _ = std::fs::remove_file(output_path);
            let reason = if output_size == 0 {
                "Empty output"
            } else {
                "Inefficient reduction"
            };
            let _ = self.db.add_decision(job_id, "skip", reason).await;
            self.update_job_state(job_id, crate::db::JobState::Skipped)
                .await?;
            return Ok(());
        }

        let mut vmaf_score = None;
        if config.quality.enable_vmaf {
            tracing::info!("[Job {}] Phase 2: Computing VMAF quality score...", job_id);
            let input_clone = input_path.to_path_buf();
            let output_clone = output_path.to_path_buf();
            let vmaf_result = tokio::task::spawn_blocking(move || {
                crate::media::ffmpeg::QualityScore::compute(&input_clone, &output_clone)
            })
            .await;

            match vmaf_result {
                Ok(Ok(score)) => {
                    vmaf_score = score.vmaf;
                    if let Some(s) = vmaf_score {
                        tracing::info!("[Job {}] VMAF Score: {:.2}", job_id, s);
                        if s < config.quality.min_vmaf_score && config.quality.revert_on_low_quality
                        {
                            tracing::warn!(
                                "Job {}: Quality gate failed ({:.2} < {}). Reverting.",
                                job_id,
                                s,
                                config.quality.min_vmaf_score
                            );
                            let _ = std::fs::remove_file(output_path);
                            let _ = self
                                .db
                                .add_decision(job_id, "skip", "Low quality (VMAF)")
                                .await;
                            self.update_job_state(job_id, crate::db::JobState::Skipped)
                                .await?;
                            return Ok(());
                        }
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("[Job {}] VMAF computation failed: {}", job_id, e);
                }
                Err(e) => {
                    tracing::warn!("[Job {}] VMAF spawn_blocking failed: {}", job_id, e);
                }
            }
        }

        let mut media_duration = metadata.duration_secs;
        if media_duration <= 0.0 {
            media_duration = crate::media::analyzer::Analyzer::probe_async(input_path)
                .await
                .ok()
                .and_then(|meta| meta.format.duration.parse::<f64>().ok())
                .unwrap_or(0.0);
        }

        let encode_speed = if encode_duration > 0.0 && media_duration > 0.0 {
            media_duration / encode_duration
        } else {
            0.0
        };

        let avg_bitrate_kbps = if media_duration > 0.0 {
            (output_size as f64 * 8.0) / (media_duration * 1000.0)
        } else {
            0.0
        };

        let _ = self
            .db
            .save_encode_stats(crate::db::EncodeStatsInput {
                job_id,
                input_size,
                output_size,
                compression_ratio: reduction,
                encode_time: encode_duration,
                encode_speed,
                avg_bitrate: avg_bitrate_kbps,
                vmaf_score,
            })
            .await;

        tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        tracing::info!("✅ Job #{} COMPLETED", job_id);
        tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        tracing::info!("  Input Size:  {} MB", input_size / 1_048_576);
        tracing::info!("  Output Size: {} MB", output_size / 1_048_576);
        tracing::info!("  Reduction:   {:.1}%", reduction * 100.0);
        if let Some(s) = vmaf_score {
            tracing::info!("  VMAF Score:  {:.2}", s);
        }
        tracing::info!("  Duration:    {:.2}s", encode_duration);
        tracing::info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        self.update_job_state(job_id, crate::db::JobState::Completed)
            .await?;
        self.update_job_progress(job_id, 100.0).await;

        self.emit_telemetry_event(TelemetryEventParams {
            telemetry_enabled,
            output_codec,
            metadata,
            event_type: "job_finished",
            status: Some("success"),
            failure_reason: None,
            input_size_bytes: Some(input_size),
            output_size_bytes: Some(output_size),
            duration_ms: Some((encode_duration * 1000.0) as u64),
            speed_factor: Some(encode_speed),
        })
        .await;

        if let Ok(file_settings) = self.db.get_file_settings().await {
            if file_settings.delete_source {
                if let Err(e) = std::fs::remove_file(input_path) {
                    tracing::warn!("Failed to delete source {:?}: {}", input_path, e);
                }
            }
        }

        Ok(())
    }

    async fn emit_telemetry_event(&self, params: TelemetryEventParams<'_>) {
        if !params.telemetry_enabled {
            return;
        }

        let hw = self.hw_info.as_ref().as_ref();
        let event = TelemetryEvent {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            event_type: params.event_type.to_string(),
            status: params.status.map(str::to_string),
            failure_reason: params.failure_reason.map(str::to_string),
            hardware_model: hardware_label(hw),
            encoder: Some(encoder_label(hw, params.output_codec)),
            video_codec: Some(params.output_codec.as_str().to_string()),
            resolution: resolution_bucket(params.metadata.width, params.metadata.height),
            duration_ms: params.duration_ms,
            input_size_bytes: params.input_size_bytes,
            output_size_bytes: params.output_size_bytes,
            speed_factor: params.speed_factor,
        };

        let _ = crate::telemetry::send_event(event).await;
    }
}

struct TelemetryEventParams<'a> {
    telemetry_enabled: bool,
    output_codec: crate::config::OutputCodec,
    metadata: &'a MediaMetadata,
    event_type: &'a str,
    status: Option<&'a str>,
    failure_reason: Option<&'a str>,
    input_size_bytes: Option<u64>,
    output_size_bytes: Option<u64>,
    duration_ms: Option<u64>,
    speed_factor: Option<f64>,
}

fn map_failure(error: &crate::error::AlchemistError) -> JobFailure {
    match error {
        crate::error::AlchemistError::EncoderUnavailable(_) => JobFailure::EncoderUnavailable,
        crate::error::AlchemistError::Analyzer(_) => JobFailure::MediaCorrupt,
        crate::error::AlchemistError::Config(_) => JobFailure::PlannerBug,
        _ => JobFailure::Transient,
    }
}
