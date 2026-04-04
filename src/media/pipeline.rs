use crate::db::Job;
use crate::error::Result;
use crate::media::analyzer::FfmpegAnalyzer;
use crate::media::executor::FfmpegExecutor;
use crate::media::planner::BasicPlanner;
use crate::orchestrator::Transcoder;
use crate::system::hardware::HardwareState;
use crate::telemetry::{TelemetryEvent, encoder_label, hardware_label, resolution_bucket};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{RwLock, broadcast};

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
    pub audio_bitrate_bps: Option<u64>,
    pub audio_channels: Option<u32>,
    pub audio_is_heavy: bool,
    pub subtitle_streams: Vec<SubtitleStreamMetadata>,
    pub audio_streams: Vec<AudioStreamMetadata>,
    pub dynamic_range: DynamicRange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubtitleStreamMetadata {
    pub stream_index: usize,
    pub codec_name: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub default: bool,
    pub forced: bool,
    pub burnable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioStreamMetadata {
    pub stream_index: usize,
    pub codec_name: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub channels: Option<u32>,
    pub default: bool,
    pub forced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredMedia {
    pub path: PathBuf,
    pub mtime: SystemTime,
    pub source_root: Option<PathBuf>,
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
        matches!(
            self,
            DynamicRange::Hdr10 | DynamicRange::Hlg | DynamicRange::DolbyVision
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TranscodeDecision {
    Skip { reason: String },
    Remux { reason: String },
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EncoderBackend {
    Cpu,
    Qsv,
    Nvenc,
    Vaapi,
    Amf,
    Videotoolbox,
}

impl EncoderBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Qsv => "qsv",
            Self::Nvenc => "nvenc",
            Self::Vaapi => "vaapi",
            Self::Amf => "amf",
            Self::Videotoolbox => "videotoolbox",
        }
    }
}

impl Encoder {
    pub fn backend(self) -> EncoderBackend {
        match self {
            Encoder::Av1Qsv | Encoder::HevcQsv | Encoder::H264Qsv => EncoderBackend::Qsv,
            Encoder::Av1Nvenc | Encoder::HevcNvenc | Encoder::H264Nvenc => EncoderBackend::Nvenc,
            Encoder::Av1Vaapi | Encoder::HevcVaapi | Encoder::H264Vaapi => EncoderBackend::Vaapi,
            Encoder::Av1Videotoolbox | Encoder::HevcVideotoolbox | Encoder::H264Videotoolbox => {
                EncoderBackend::Videotoolbox
            }
            Encoder::Av1Amf | Encoder::HevcAmf | Encoder::H264Amf => EncoderBackend::Amf,
            Encoder::Av1Svt | Encoder::Av1Aom | Encoder::HevcX265 | Encoder::H264X264 => {
                EncoderBackend::Cpu
            }
        }
    }

    pub fn output_codec(self) -> crate::config::OutputCodec {
        match self {
            Encoder::Av1Qsv
            | Encoder::Av1Nvenc
            | Encoder::Av1Vaapi
            | Encoder::Av1Videotoolbox
            | Encoder::Av1Amf
            | Encoder::Av1Svt
            | Encoder::Av1Aom => crate::config::OutputCodec::Av1,
            Encoder::HevcQsv
            | Encoder::HevcNvenc
            | Encoder::HevcVaapi
            | Encoder::HevcVideotoolbox
            | Encoder::HevcAmf
            | Encoder::HevcX265 => crate::config::OutputCodec::Hevc,
            Encoder::H264Qsv
            | Encoder::H264Nvenc
            | Encoder::H264Vaapi
            | Encoder::H264Videotoolbox
            | Encoder::H264Amf
            | Encoder::H264X264 => crate::config::OutputCodec::H264,
        }
    }

    pub fn ffmpeg_encoder_name(self) -> &'static str {
        match self {
            Encoder::Av1Qsv => "av1_qsv",
            Encoder::Av1Nvenc => "av1_nvenc",
            Encoder::Av1Vaapi => "av1_vaapi",
            Encoder::Av1Videotoolbox => "av1_videotoolbox",
            Encoder::Av1Amf => "av1_amf",
            Encoder::Av1Svt => "libsvtav1",
            Encoder::Av1Aom => "libaom-av1",
            Encoder::HevcQsv => "hevc_qsv",
            Encoder::HevcNvenc => "hevc_nvenc",
            Encoder::HevcVaapi => "hevc_vaapi",
            Encoder::HevcVideotoolbox => "hevc_videotoolbox",
            Encoder::HevcAmf => "hevc_amf",
            Encoder::HevcX265 => "libx265",
            Encoder::H264Qsv => "h264_qsv",
            Encoder::H264Nvenc => "h264_nvenc",
            Encoder::H264Vaapi => "h264_vaapi",
            Encoder::H264Videotoolbox => "h264_videotoolbox",
            Encoder::H264Amf => "h264_amf",
            Encoder::H264X264 => "libx264",
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FallbackKind {
    Codec,
    Backend,
    Cpu,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlannedFallback {
    pub kind: FallbackKind,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioCodec {
    Aac,
    Opus,
}

impl AudioCodec {
    pub fn ffmpeg_name(&self) -> &'static str {
        match self {
            Self::Aac => "aac",
            Self::Opus => "libopus",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioStreamPlan {
    Copy,
    Transcode {
        codec: AudioCodec,
        bitrate_kbps: u16,
        channels: Option<u32>,
    },
    Drop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleStreamPlan {
    CopyAllCompatible,
    Drop,
    Burn { stream_index: usize },
    Extract { outputs: Vec<SidecarOutputPlan> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SidecarOutputPlan {
    pub stream_index: usize,
    pub codec: String,
    pub final_path: PathBuf,
    pub temp_path: PathBuf,
}

impl SubtitleStreamPlan {
    pub fn sidecar_outputs(&self) -> &[SidecarOutputPlan] {
        match self {
            SubtitleStreamPlan::Extract { outputs } => outputs.as_slice(),
            _ => &[],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum FilterStep {
    Tonemap {
        algorithm: crate::config::TonemapAlgorithm,
        peak: f32,
        desat: f32,
    },
    Format {
        pixel_format: String,
    },
    SubtitleBurn {
        stream_index: usize,
    },
    HwUpload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodePlan {
    pub decision: TranscodeDecision,
    pub is_remux: bool,
    pub output_path: Option<PathBuf>,
    pub container: String,
    pub requested_codec: crate::config::OutputCodec,
    pub output_codec: Option<crate::config::OutputCodec>,
    pub encoder: Option<Encoder>,
    pub backend: Option<EncoderBackend>,
    pub rate_control: Option<RateControl>,
    pub encoder_preset: Option<String>,
    pub threads: usize,
    pub audio: AudioStreamPlan,
    /// If Some, only these audio stream indices are mapped.
    /// If None, all audio streams are mapped (default behavior).
    pub audio_stream_indices: Option<Vec<usize>>,
    pub subtitles: SubtitleStreamPlan,
    pub filters: Vec<FilterStep>,
    pub allow_fallback: bool,
    pub fallback: Option<PlannedFallback>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub requested_codec: crate::config::OutputCodec,
    pub planned_output_codec: crate::config::OutputCodec,
    pub requested_encoder: Option<Encoder>,
    pub used_encoder: Option<Encoder>,
    pub used_backend: Option<EncoderBackend>,
    pub fallback: Option<PlannedFallback>,
    pub fallback_occurred: bool,
    pub actual_output_codec: Option<crate::config::OutputCodec>,
    pub actual_encoder_name: Option<String>,
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

#[allow(async_fn_in_trait)]
pub trait Analyzer: Send + Sync {
    async fn analyze(&self, path: &Path) -> Result<MediaAnalysis>;
}

#[allow(async_fn_in_trait)]
pub trait Planner: Send + Sync {
    async fn plan(
        &self,
        analysis: &MediaAnalysis,
        output_path: &Path,
        profile: Option<&crate::db::LibraryProfile>,
    ) -> Result<TranscodePlan>;
}

#[allow(async_fn_in_trait)]
pub trait Executor: Send + Sync {
    async fn execute(
        &self,
        job: &Job,
        plan: &TranscodePlan,
        analysis: &MediaAnalysis,
    ) -> Result<ExecutionResult>;
}

pub struct Pipeline {
    db: Arc<crate::db::Db>,
    orchestrator: Arc<Transcoder>,
    config: Arc<RwLock<crate::config::Config>>,
    hardware_state: HardwareState,
    tx: Arc<broadcast::Sender<crate::db::AlchemistEvent>>,
    event_channels: Arc<crate::db::EventChannels>,
    dry_run: bool,
}

struct FinalizeJobContext<'a> {
    output_path: &'a Path,
    temp_output_path: &'a Path,
    plan: &'a TranscodePlan,
    start_time: std::time::Instant,
    metadata: &'a MediaMetadata,
    execution_result: &'a ExecutionResult,
}

struct FinalizeFailureContext<'a> {
    plan: &'a TranscodePlan,
    metadata: &'a MediaMetadata,
    execution_result: &'a ExecutionResult,
    config_snapshot: &'a crate::config::Config,
    start_time: std::time::Instant,
    temp_output_path: &'a Path,
}

impl Pipeline {
    pub fn new(
        db: Arc<crate::db::Db>,
        orchestrator: Arc<Transcoder>,
        config: Arc<RwLock<crate::config::Config>>,
        hardware_state: HardwareState,
        tx: Arc<broadcast::Sender<crate::db::AlchemistEvent>>,
        event_channels: Arc<crate::db::EventChannels>,
        dry_run: bool,
    ) -> Self {
        Self {
            db,
            orchestrator,
            config,
            hardware_state,
            tx,
            event_channels,
            dry_run,
        }
    }

    pub async fn enqueue_discovered(&self, discovered: DiscoveredMedia) -> Result<()> {
        let _ = enqueue_discovered_with_db(&self.db, discovered).await?;
        Ok(())
    }
}

pub async fn enqueue_discovered_with_db(
    db: &crate::db::Db,
    discovered: DiscoveredMedia,
) -> Result<bool> {
    let settings = match db.get_file_settings().await {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to fetch file settings, using defaults: {}", e);
            default_file_settings()
        }
    };

    if let Some(reason) = skip_reason_for_discovered_path(db, &discovered.path, &settings).await? {
        tracing::info!("Skipping {:?} ({})", discovered.path, reason);
        return Ok(false);
    }

    let output_path =
        settings.output_path_for_source(&discovered.path, discovered.source_root.as_deref());
    if output_path.exists() && !settings.should_replace_existing_output() {
        tracing::info!(
            "Skipping {:?} (output exists, replace_strategy = keep)",
            discovered.path
        );
        return Ok(false);
    }

    db.enqueue_job(&discovered.path, &output_path, discovered.mtime)
        .await
}

fn default_file_settings() -> crate::db::FileSettings {
    crate::db::FileSettings {
        id: 1,
        delete_source: false,
        output_extension: "mkv".to_string(),
        output_suffix: "-alchemist".to_string(),
        replace_strategy: "keep".to_string(),
        output_root: None,
    }
}

fn matches_generated_output_pattern(path: &Path, settings: &crate::db::FileSettings) -> bool {
    let expected_extension = settings.output_extension.trim_start_matches('.');
    if !expected_extension.is_empty() {
        let actual_extension = match path.extension().and_then(|extension| extension.to_str()) {
            Some(extension) => extension,
            None => return false,
        };
        if !actual_extension.eq_ignore_ascii_case(expected_extension) {
            return false;
        }
    }

    let suffix = if settings.output_suffix.is_empty() {
        "-alchemist"
    } else {
        settings.output_suffix.as_str()
    };

    path.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.ends_with(suffix))
}

async fn skip_reason_for_discovered_path(
    db: &crate::db::Db,
    path: &Path,
    settings: &crate::db::FileSettings,
) -> Result<Option<&'static str>> {
    if matches_generated_output_pattern(path, settings) {
        return Ok(Some("matches generated output naming pattern"));
    }

    let path_string = path.to_string_lossy();
    if db.has_job_with_output_path(path_string.as_ref()).await? {
        return Ok(Some("already tracked as a job output"));
    }

    Ok(None)
}

/// Creates a temporary output path for encoding.
/// Uses a predictable `.alchemist.tmp` suffix - this is acceptable because:
/// 1. The suffix is unique to Alchemist and unlikely to conflict
/// 2. Files are created in user-owned media directories
/// 3. Same-file concurrent transcodes are prevented at the job level
fn temp_output_path_for(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let filename = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    parent.join(format!("{filename}.alchemist.tmp"))
}

impl Pipeline {
    /// Runs only the analysis and planning phases for a job.
    /// Does not execute any encode. Used by the startup
    /// auto-analyzer to populate skip/transcode decisions.
    pub async fn analyze_job_only(&self, job: crate::db::Job) -> Result<()> {
        let job_id = job.id;

        // Update status to analyzing
        self.db
            .update_job_status(job_id, crate::db::JobState::Analyzing)
            .await?;

        // Run ffprobe analysis
        let analyzer = crate::media::analyzer::FfmpegAnalyzer;
        let analysis = match analyzer
            .analyze(std::path::Path::new(&job.input_path))
            .await
        {
            Ok(a) => a,
            Err(e) => {
                let reason = format!("analysis_failed|error={e}");
                let _ = self.db.add_log("error", Some(job_id), &reason).await;
                self.db.add_decision(job_id, "skip", &reason).await.ok();
                self.db
                    .update_job_status(job_id, crate::db::JobState::Failed)
                    .await?;
                return Ok(());
            }
        };

        // Get the output path for planning
        let output_path = std::path::PathBuf::from(&job.output_path);

        // Get profile for this job's input path (if any)
        let profile = match self.db.get_profile_for_path(&job.input_path).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Failed to fetch profile for {}: {}", job.input_path, e);
                None
            }
        };

        // Run the planner
        let config_snapshot = Arc::new(self.config.read().await.clone());
        let hw_info = self.hardware_state.snapshot().await;
        let planner = crate::media::planner::BasicPlanner::new(config_snapshot, hw_info);
        let plan = match planner
            .plan(&analysis, &output_path, profile.as_ref())
            .await
        {
            Ok(p) => p,
            Err(e) => {
                let reason = format!("planning_failed|error={e}");
                let _ = self.db.add_log("error", Some(job_id), &reason).await;
                self.db.add_decision(job_id, "skip", &reason).await.ok();
                self.db
                    .update_job_status(job_id, crate::db::JobState::Failed)
                    .await?;
                return Ok(());
            }
        };

        // Store the decision and return to queued — do NOT encode
        match &plan.decision {
            crate::media::pipeline::TranscodeDecision::Skip { reason } => {
                self.db.add_decision(job_id, "skip", reason).await.ok();
                self.db
                    .update_job_status(job_id, crate::db::JobState::Skipped)
                    .await?;
            }
            crate::media::pipeline::TranscodeDecision::Remux { reason } => {
                self.db.add_decision(job_id, "transcode", reason).await.ok();
                // Leave as queued — will be picked up for remux when engine starts
                self.db
                    .update_job_status(job_id, crate::db::JobState::Queued)
                    .await?;
            }
            crate::media::pipeline::TranscodeDecision::Transcode { reason } => {
                self.db.add_decision(job_id, "transcode", reason).await.ok();
                // Leave as queued — will be picked up for encoding when engine starts
                self.db
                    .update_job_status(job_id, crate::db::JobState::Queued)
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn process_job(&self, job: Job) -> std::result::Result<(), JobFailure> {
        let file_path = PathBuf::from(&job.input_path);

        let file_settings = match self.db.get_file_settings().await {
            Ok(settings) => settings,
            Err(e) => {
                tracing::error!("Failed to fetch file settings, using defaults: {}", e);
                default_file_settings()
            }
        };

        let output_path = PathBuf::from(&job.output_path);
        let temp_output_path = temp_output_path_for(&output_path);

        if file_path == output_path {
            tracing::error!(
                "Job {}: Output path matches input path; refusing to overwrite source.",
                job.id
            );
            let _ = self
                .db
                .add_decision(job.id, "skip", "Output path matches input path")
                .await;
            let _ = self
                .update_job_state(job.id, crate::db::JobState::Skipped)
                .await;
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
            let _ = self
                .update_job_state(job.id, crate::db::JobState::Skipped)
                .await;
            return Ok(());
        }

        if temp_output_path.exists() {
            if let Err(err) = std::fs::remove_file(&temp_output_path) {
                tracing::warn!(
                    "Job {}: Failed to remove stale temp output {:?}: {}",
                    job.id,
                    temp_output_path,
                    err
                );
            }
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
                let msg = format!("Probing failed: {e}");
                tracing::error!("Job {}: {}", job.id, msg);
                let _ = self.db.add_log("error", Some(job.id), &msg).await;
                let _ = self
                    .update_job_state(job.id, crate::db::JobState::Failed)
                    .await;
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

        match self.should_stop_job(job.id).await {
            Ok(true) => {
                tracing::info!("Job {} was cancelled before encode planning.", job.id);
                return Ok(());
            }
            Ok(false) => {}
            Err(_) => return Err(JobFailure::Transient),
        }

        let config_snapshot = self.config.read().await.clone();
        let hw_info = self.hardware_state.snapshot().await;
        let planner = BasicPlanner::new(Arc::new(config_snapshot.clone()), hw_info.clone());
        let profile = match self.db.get_profile_for_path(&job.input_path).await {
            Ok(profile) => profile,
            Err(err) => {
                let msg = format!("Failed to resolve library profile: {err}");
                tracing::error!("Job {}: {}", job.id, msg);
                let _ = self.db.add_log("error", Some(job.id), &msg).await;
                let _ = self
                    .update_job_state(job.id, crate::db::JobState::Failed)
                    .await;
                return Err(JobFailure::Transient);
            }
        };
        let mut plan = match planner
            .plan(&analysis, &output_path, profile.as_ref())
            .await
        {
            Ok(plan) => plan,
            Err(e) => {
                let msg = format!("Planner failed: {e}");
                tracing::error!("Job {}: {}", job.id, msg);
                let _ = self.db.add_log("error", Some(job.id), &msg).await;
                let _ = self
                    .update_job_state(job.id, crate::db::JobState::Failed)
                    .await;
                return Err(JobFailure::PlannerBug);
            }
        };

        if !matches!(plan.decision, TranscodeDecision::Skip { .. }) {
            plan.output_path = Some(temp_output_path.clone());
        }

        for sidecar_output in plan.subtitles.sidecar_outputs() {
            if sidecar_output.temp_path.exists() {
                if let Err(err) = std::fs::remove_file(&sidecar_output.temp_path) {
                    tracing::warn!(
                        "Job {}: Failed to remove stale subtitle temp output {:?}: {}",
                        job.id,
                        sidecar_output.temp_path,
                        err
                    );
                }
            }
        }

        let (should_execute, action, reason, next_status) = match &plan.decision {
            TranscodeDecision::Transcode { reason } => (
                true,
                "encode",
                reason.clone(),
                crate::db::JobState::Encoding,
            ),
            TranscodeDecision::Remux { .. } => {
                tracing::info!(
                    "Job {}: Remuxing MP4→MKV (stream copy, no re-encode)",
                    job.id
                );
                (
                    true,
                    "remux",
                    "remux: mp4_to_mkv_stream_copy".to_string(),
                    crate::db::JobState::Remuxing,
                )
            }
            TranscodeDecision::Skip { reason } => {
                (false, "skip", reason.clone(), crate::db::JobState::Skipped)
            }
        };

        if !should_execute {
            tracing::info!("Decision: SKIP Job {} - {}", job.id, &reason);
            let _ = self.db.add_decision(job.id, "skip", &reason).await;
            let _ = self
                .update_job_state(job.id, crate::db::JobState::Skipped)
                .await;
            return Ok(());
        }

        tracing::info!(
            "Decision: {} Job {} - {}",
            action.to_ascii_uppercase(),
            job.id,
            &reason
        );
        let _ = self.db.add_decision(job.id, action, &reason).await;
        let _ = self.tx.send(crate::db::AlchemistEvent::Decision {
            job_id: job.id,
            action: action.to_string(),
            reason: reason.clone(),
        });

        if self.update_job_state(job.id, next_status).await.is_err() {
            return Err(JobFailure::Transient);
        }
        self.update_job_progress(job.id, 0.0).await;

        match self.should_stop_job(job.id).await {
            Ok(true) => {
                tracing::info!("Job {} was cancelled before FFmpeg execution.", job.id);
                return Ok(());
            }
            Ok(false) => {}
            Err(_) => return Err(JobFailure::Transient),
        }

        self.emit_telemetry_event(TelemetryEventParams {
            telemetry_enabled: config_snapshot.system.enable_telemetry,
            output_codec: config_snapshot.transcode.output_codec,
            encoder_override: None,
            fallback: plan.fallback.as_ref(),
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
            self.db.clone(),
            hw_info.clone(),
            self.tx.clone(),
            self.event_channels.clone(),
            self.dry_run,
        );

        match executor.execute(&job, &plan, &analysis).await {
            Ok(result) => {
                if result.fallback_occurred && !plan.allow_fallback {
                    tracing::error!("Job {}: Encoder fallback detected and not allowed.", job.id);
                    let _ = self
                        .update_job_state(job.id, crate::db::JobState::Failed)
                        .await;
                    return Err(JobFailure::EncoderUnavailable);
                }

                if let Err(err) = self
                    .finalize_job(
                        job.clone(),
                        &file_path,
                        FinalizeJobContext {
                            output_path: &output_path,
                            temp_output_path: &temp_output_path,
                            plan: &plan,
                            start_time,
                            metadata,
                            execution_result: &result,
                        },
                    )
                    .await
                {
                    self.handle_finalize_failure(
                        job.id,
                        FinalizeFailureContext {
                            plan: &plan,
                            metadata,
                            execution_result: &result,
                            config_snapshot: &config_snapshot,
                            start_time,
                            temp_output_path: &temp_output_path,
                        },
                        &err,
                    )
                    .await;
                    return Err(JobFailure::Transient);
                }

                Ok(())
            }
            Err(e) => {
                if temp_output_path.exists() {
                    if let Err(err) = tokio::fs::remove_file(&temp_output_path).await {
                        tracing::warn!(
                            "Job {}: Failed to remove partial output {:?}: {}",
                            job.id,
                            temp_output_path,
                            err
                        );
                    } else {
                        tracing::info!(
                            "Job {}: Removed partial output {:?}",
                            job.id,
                            temp_output_path
                        );
                    }
                }
                cleanup_temp_subtitle_output(job.id, &plan).await;
                let failure_reason = if let crate::error::AlchemistError::Cancelled = e {
                    "cancelled"
                } else {
                    "transcode_failed"
                };
                self.emit_telemetry_event(TelemetryEventParams {
                    telemetry_enabled: config_snapshot.system.enable_telemetry,
                    output_codec: config_snapshot.transcode.output_codec,
                    encoder_override: None,
                    fallback: plan.fallback.as_ref(),
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
                    let _ = self
                        .update_job_state(job.id, crate::db::JobState::Cancelled)
                        .await;
                } else {
                    let msg = format!("Transcode failed: {e}");
                    tracing::error!("Job {}: {}", job.id, msg);
                    let _ = self.db.add_log("error", Some(job.id), &msg).await;
                    let _ = self
                        .update_job_state(job.id, crate::db::JobState::Failed)
                        .await;
                }
                Err(map_failure(&e))
            }
        }
    }

    async fn update_job_state(&self, job_id: i64, status: crate::db::JobState) -> Result<()> {
        if let Err(e) = self.db.update_job_status(job_id, status).await {
            tracing::error!("Failed to update job {} status {:?}: {}", job_id, status, e);
            return Err(e);
        }
        let _ = self
            .tx
            .send(crate::db::AlchemistEvent::JobStateChanged { job_id, status });
        Ok(())
    }

    async fn update_job_progress(&self, job_id: i64, progress: f64) {
        if let Err(e) = self.db.update_job_progress(job_id, progress).await {
            tracing::error!("Failed to update job progress: {}", e);
            return;
        }
        let _ = self.tx.send(crate::db::AlchemistEvent::Progress {
            job_id,
            percentage: progress,
            time: String::new(),
        });
    }

    async fn should_stop_job(&self, job_id: i64) -> Result<bool> {
        let Some(job) = self.db.get_job_by_id(job_id).await? else {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        };
        Ok(job.status == crate::db::JobState::Cancelled)
    }

    fn promote_temp_artifact(&self, temp_path: &Path, final_path: &Path) -> Result<()> {
        if cfg!(windows) && final_path.exists() {
            std::fs::remove_file(final_path)?;
        }
        std::fs::rename(temp_path, final_path)?;
        Ok(())
    }

    async fn finalize_job(
        &self,
        job: Job,
        input_path: &Path,
        context: FinalizeJobContext<'_>,
    ) -> Result<()> {
        let job_id = job.id;
        let input_metadata = std::fs::metadata(input_path)?;
        let input_size = input_metadata.len();

        let output_metadata = std::fs::metadata(context.temp_output_path)?;
        let output_size = output_metadata.len();

        if input_size == 0 {
            tracing::error!("Job {}: Input file is empty. Finalizing as failed.", job_id);
            let _ = std::fs::remove_file(context.temp_output_path);
            cleanup_temp_subtitle_output(job_id, context.plan).await;

            self.update_job_state(job_id, crate::db::JobState::Failed)
                .await?;
            return Ok(());
        }

        let reduction = 1.0 - (output_size as f64 / input_size as f64);
        let encode_duration = context.start_time.elapsed().as_secs_f64();

        let config = self.config.read().await;
        let telemetry_enabled = config.system.enable_telemetry;

        if output_size == 0
            || (!context.plan.is_remux && reduction < config.transcode.size_reduction_threshold)
        {
            tracing::warn!(
                "Job {}: Size reduction gate failed ({:.2}%). Reverting.",
                job_id,
                reduction * 100.0
            );
            let _ = std::fs::remove_file(context.temp_output_path);
            cleanup_temp_subtitle_output(job_id, context.plan).await;
            let reason = if output_size == 0 {
                format!(
                    "size_reduction_insufficient|reduction=0.000,threshold={:.3},output_size=0",
                    config.transcode.size_reduction_threshold
                )
            } else {
                format!(
                    "size_reduction_insufficient|reduction={:.3},threshold={:.3},output_size={}",
                    reduction, config.transcode.size_reduction_threshold, output_size
                )
            };
            let _ = self.db.add_decision(job_id, "skip", &reason).await;
            self.update_job_state(job_id, crate::db::JobState::Skipped)
                .await?;
            return Ok(());
        }

        let mut vmaf_score = None;
        if !context.plan.is_remux && config.quality.enable_vmaf {
            tracing::info!("[Job {}] Phase 2: Computing VMAF quality score...", job_id);
            let input_clone = input_path.to_path_buf();
            let output_clone = context.temp_output_path.to_path_buf();
            let vmaf_result = tokio::task::spawn_blocking(move || {
                crate::media::ffmpeg::QualityScore::compute(&input_clone, &output_clone)
            })
            .await;

            match vmaf_result {
                Ok(Ok(score)) => {
                    vmaf_score = score.vmaf;
                    if let Some(s) = vmaf_score {
                        tracing::info!("[Job {}] VMAF Score: {:.2}", job_id, s);
                        if let Some(threshold) = config.transcode.vmaf_min_score {
                            if s < threshold {
                                let _ = std::fs::remove_file(context.temp_output_path);
                                cleanup_temp_subtitle_output(job_id, context.plan).await;
                                return Err(crate::error::AlchemistError::QualityCheckFailed(
                                    format!(
                                        "VMAF score {:.1} fell below the minimum threshold of {:.1}. The original file has been preserved.",
                                        s, threshold
                                    ),
                                ));
                            }
                        }
                        if s < config.quality.min_vmaf_score && config.quality.revert_on_low_quality
                        {
                            tracing::warn!(
                                "Job {}: Quality gate failed ({:.2} < {}). Reverting.",
                                job_id,
                                s,
                                config.quality.min_vmaf_score
                            );
                            let _ = std::fs::remove_file(context.temp_output_path);
                            cleanup_temp_subtitle_output(job_id, context.plan).await;
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

        let mut media_duration = context.metadata.duration_secs;
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

        self.db
            .save_encode_stats(crate::db::EncodeStatsInput {
                job_id,
                input_size,
                output_size,
                compression_ratio: reduction,
                encode_time: encode_duration,
                encode_speed,
                avg_bitrate: avg_bitrate_kbps,
                vmaf_score,
                output_codec: Some(
                    context
                        .execution_result
                        .actual_output_codec
                        .unwrap_or(context.execution_result.planned_output_codec)
                        .as_str()
                        .to_string(),
                ),
            })
            .await?;

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

        if !context.plan.subtitles.sidecar_outputs().is_empty() {
            let mut promoted_sidecars: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
            for sidecar_output in context.plan.subtitles.sidecar_outputs() {
                if let Err(err) = self
                    .promote_temp_artifact(&sidecar_output.temp_path, &sidecar_output.final_path)
                {
                    for (temp, final_path) in &promoted_sidecars {
                        let _ = std::fs::rename(final_path, temp);
                    }
                    return Err(err);
                }
                promoted_sidecars.push((
                    sidecar_output.temp_path.clone(),
                    sidecar_output.final_path.clone(),
                ));
            }
            if let Err(err) =
                self.promote_temp_artifact(context.temp_output_path, context.output_path)
            {
                for (temp, final_path) in &promoted_sidecars {
                    let _ = std::fs::rename(final_path, temp);
                }
                return Err(err);
            }
        } else {
            self.promote_temp_artifact(context.temp_output_path, context.output_path)?;
        }
        self.update_job_state(job_id, crate::db::JobState::Completed)
            .await?;
        self.update_job_progress(job_id, 100.0).await;

        self.emit_telemetry_event(TelemetryEventParams {
            telemetry_enabled,
            output_codec: context
                .execution_result
                .actual_output_codec
                .unwrap_or(context.execution_result.planned_output_codec),
            encoder_override: context.execution_result.actual_encoder_name.as_deref(),
            fallback: context.execution_result.fallback.as_ref(),
            metadata: context.metadata,
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
                // Safety: verify the promoted output is intact before destroying the source.
                // This prevents data loss if the filesystem silently corrupted the output
                // during rename (e.g., stale NFS/SMB mount, full disk).
                match std::fs::metadata(context.output_path) {
                    Ok(m) if m.len() > 0 => {
                        if let Err(e) = std::fs::remove_file(input_path) {
                            tracing::warn!("Failed to delete source {:?}: {}", input_path, e);
                        }
                    }
                    Ok(_) => {
                        tracing::error!(
                            "Job {}: Output file {:?} is empty after promotion — source preserved to prevent data loss",
                            job_id, context.output_path
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "Job {}: Cannot verify output {:?} after promotion ({}). Source preserved to prevent data loss",
                            job_id, context.output_path, e
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_finalize_failure(
        &self,
        job_id: i64,
        context: FinalizeFailureContext<'_>,
        err: &crate::error::AlchemistError,
    ) {
        tracing::error!("Job {}: Finalization failed: {}", job_id, err);

        let message = format!("Finalization failed: {err}");
        let _ = self.db.add_log("error", Some(job_id), &message).await;
        if let crate::error::AlchemistError::QualityCheckFailed(reason) = err {
            let _ = self.db.add_decision(job_id, "reject", reason).await;
        }

        if context.temp_output_path.exists() {
            if let Err(cleanup_err) = tokio::fs::remove_file(context.temp_output_path).await {
                tracing::warn!(
                    "Job {}: Failed to remove temp output after finalize error {:?}: {}",
                    job_id,
                    context.temp_output_path,
                    cleanup_err
                );
            }
        }
        cleanup_temp_subtitle_output(job_id, context.plan).await;

        self.emit_telemetry_event(TelemetryEventParams {
            telemetry_enabled: context.config_snapshot.system.enable_telemetry,
            output_codec: context
                .execution_result
                .actual_output_codec
                .unwrap_or(context.execution_result.planned_output_codec),
            encoder_override: context.execution_result.actual_encoder_name.as_deref(),
            fallback: context.plan.fallback.as_ref(),
            metadata: context.metadata,
            event_type: "job_finished",
            status: Some("failure"),
            failure_reason: Some("finalize_failed"),
            input_size_bytes: Some(context.metadata.size_bytes),
            output_size_bytes: None,
            duration_ms: Some(context.start_time.elapsed().as_millis() as u64),
            speed_factor: None,
        })
        .await;

        let _ = self
            .update_job_state(job_id, crate::db::JobState::Failed)
            .await;
    }

    async fn emit_telemetry_event(&self, params: TelemetryEventParams<'_>) {
        if !params.telemetry_enabled {
            return;
        }

        let hw_snapshot = self.hardware_state.snapshot().await;
        let hw = hw_snapshot.as_ref();
        let event = TelemetryEvent {
            app_version: crate::version::current().to_string(),
            event_type: params.event_type.to_string(),
            status: params.status.map(str::to_string),
            failure_reason: params.failure_reason.map(str::to_string),
            fallback_kind: params.fallback.map(|fallback| match fallback.kind {
                FallbackKind::Codec => "codec".to_string(),
                FallbackKind::Backend => "backend".to_string(),
                FallbackKind::Cpu => "cpu".to_string(),
            }),
            hardware_model: hardware_label(hw),
            encoder: Some(
                params
                    .encoder_override
                    .map(str::to_string)
                    .unwrap_or_else(|| encoder_label(hw, params.output_codec)),
            ),
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
    encoder_override: Option<&'a str>,
    fallback: Option<&'a PlannedFallback>,
    metadata: &'a MediaMetadata,
    event_type: &'a str,
    status: Option<&'a str>,
    failure_reason: Option<&'a str>,
    input_size_bytes: Option<u64>,
    output_size_bytes: Option<u64>,
    duration_ms: Option<u64>,
    speed_factor: Option<f64>,
}

async fn cleanup_temp_subtitle_output(job_id: i64, plan: &TranscodePlan) {
    for sidecar_output in plan.subtitles.sidecar_outputs() {
        if sidecar_output.temp_path.exists() {
            if let Err(err) = tokio::fs::remove_file(&sidecar_output.temp_path).await {
                tracing::warn!(
                    "Job {}: Failed to remove subtitle temp output {:?}: {}",
                    job_id,
                    sidecar_output.temp_path,
                    err
                );
            }
        }
    }
}

fn map_failure(error: &crate::error::AlchemistError) -> JobFailure {
    match error {
        crate::error::AlchemistError::EncoderUnavailable(_) => JobFailure::EncoderUnavailable,
        crate::error::AlchemistError::Analyzer(_) => JobFailure::MediaCorrupt,
        crate::error::AlchemistError::Config(_) => JobFailure::PlannerBug,
        _ => JobFailure::Transient,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Transcoder;
    use crate::db::Db;
    use crate::system::hardware::{HardwareInfo, HardwareState, Vendor};
    use std::sync::Arc;
    use tokio::sync::{RwLock, broadcast};

    #[test]
    fn generated_output_pattern_matches_default_suffix() {
        let settings = default_file_settings();
        assert!(matches_generated_output_pattern(
            Path::new("/media/movie-alchemist.mkv"),
            &settings,
        ));
        assert!(!matches_generated_output_pattern(
            Path::new("/media/movie.mkv"),
            &settings,
        ));
    }

    #[tokio::test]
    async fn enqueue_discovered_rejects_known_output_paths()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        db_path.push(format!(
            "alchemist_output_filter_{}.db",
            rand::random::<u64>()
        ));
        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        db.update_file_settings(false, "mkv", "", "keep", None)
            .await?;

        let input = Path::new("/library/movie.mkv");
        let output = Path::new("/library/movie-alchemist.mkv");
        let _ = db
            .enqueue_job(input, output, SystemTime::UNIX_EPOCH)
            .await?;

        let changed = enqueue_discovered_with_db(
            &db,
            DiscoveredMedia {
                path: output.to_path_buf(),
                mtime: SystemTime::UNIX_EPOCH,
                source_root: None,
            },
        )
        .await?;
        assert!(!changed);

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn cleanup_temp_subtitle_output_removes_sidecar_temp() -> anyhow::Result<()> {
        let temp_root = std::env::temp_dir().join(format!(
            "alchemist_sidecar_cleanup_{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&temp_root)?;
        let temp_sidecar = temp_root.join("movie.subs.mks.alchemist-part");
        std::fs::write(&temp_sidecar, b"sidecar")?;

        let plan = TranscodePlan {
            decision: TranscodeDecision::Transcode {
                reason: "test".to_string(),
            },
            is_remux: false,
            output_path: None,
            container: "mkv".to_string(),
            requested_codec: crate::config::OutputCodec::H264,
            output_codec: Some(crate::config::OutputCodec::H264),
            encoder: Some(Encoder::H264X264),
            backend: Some(EncoderBackend::Cpu),
            rate_control: Some(RateControl::Crf { value: 21 }),
            encoder_preset: Some("medium".to_string()),
            threads: 0,
            audio: AudioStreamPlan::Copy,
            audio_stream_indices: None,
            subtitles: SubtitleStreamPlan::Extract {
                outputs: vec![SidecarOutputPlan {
                    stream_index: 0,
                    codec: "srt".to_string(),
                    final_path: temp_root.join("movie.eng.srt"),
                    temp_path: temp_sidecar.clone(),
                }],
            },
            filters: Vec::new(),
            allow_fallback: true,
            fallback: None,
        };

        cleanup_temp_subtitle_output(1, &plan).await;
        assert!(!temp_sidecar.exists());

        let _ = std::fs::remove_dir_all(temp_root);
        Ok(())
    }

    #[tokio::test]
    async fn finalize_failure_marks_job_failed_and_cleans_temp_output() -> anyhow::Result<()> {
        let db_path = std::env::temp_dir().join(format!(
            "alchemist_finalize_failure_{}.db",
            rand::random::<u64>()
        ));
        let temp_root = std::env::temp_dir().join(format!(
            "alchemist_finalize_failure_{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&temp_root)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let input = temp_root.join("movie.mkv");
        let output = temp_root.join("movie-alchemist.mkv");
        std::fs::write(&input, b"source")?;

        let _ = db
            .enqueue_job(&input, &output, SystemTime::UNIX_EPOCH)
            .await?;
        let job = db
            .get_job_by_input_path(input.to_string_lossy().as_ref())
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing queued job"))?;
        db.update_job_status(job.id, crate::db::JobState::Encoding)
            .await?;

        let temp_output = temp_output_path_for(&output);
        std::fs::write(&temp_output, b"partial")?;

        let config = Arc::new(RwLock::new(crate::config::Config::default()));
        let hardware_state = HardwareState::new(Some(HardwareInfo {
            vendor: Vendor::Cpu,
            device_path: None,
            supported_codecs: vec!["av1".to_string(), "hevc".to_string(), "h264".to_string()],
            backends: Vec::new(),
            detection_notes: Vec::new(),
            selection_reason: String::new(),
            probe_summary: crate::system::hardware::ProbeSummary::default(),
        }));
        let (tx, _rx) = broadcast::channel(8);
        let (jobs_tx, _) = broadcast::channel(100);
        let (config_tx, _) = broadcast::channel(10);
        let (system_tx, _) = broadcast::channel(10);
        let event_channels = Arc::new(crate::db::EventChannels {
            jobs: jobs_tx,
            config: config_tx,
            system: system_tx,
        });
        let pipeline = Pipeline::new(
            db.clone(),
            Arc::new(Transcoder::new()),
            config.clone(),
            hardware_state,
            Arc::new(tx),
            event_channels,
            true,
        );

        let plan = TranscodePlan {
            decision: TranscodeDecision::Transcode {
                reason: "test".to_string(),
            },
            is_remux: false,
            output_path: Some(temp_output.clone()),
            container: "mkv".to_string(),
            requested_codec: crate::config::OutputCodec::H264,
            output_codec: Some(crate::config::OutputCodec::H264),
            encoder: Some(Encoder::H264X264),
            backend: Some(EncoderBackend::Cpu),
            rate_control: Some(RateControl::Crf { value: 21 }),
            encoder_preset: Some("medium".to_string()),
            threads: 0,
            audio: AudioStreamPlan::Copy,
            audio_stream_indices: None,
            subtitles: SubtitleStreamPlan::Drop,
            filters: Vec::new(),
            allow_fallback: true,
            fallback: None,
        };
        let metadata = MediaMetadata {
            path: input.clone(),
            duration_secs: 12.0,
            codec_name: "h264".to_string(),
            width: 1920,
            height: 1080,
            bit_depth: Some(8),
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            size_bytes: 2_000,
            video_bitrate_bps: Some(5_000_000),
            container_bitrate_bps: Some(5_500_000),
            fps: 24.0,
            container: "mkv".to_string(),
            audio_codec: Some("aac".to_string()),
            audio_bitrate_bps: Some(192_000),
            audio_channels: Some(2),
            audio_is_heavy: false,
            subtitle_streams: Vec::new(),
            audio_streams: Vec::new(),
            dynamic_range: DynamicRange::Sdr,
        };
        let result = ExecutionResult {
            requested_codec: crate::config::OutputCodec::H264,
            planned_output_codec: crate::config::OutputCodec::H264,
            requested_encoder: Some(Encoder::H264X264),
            used_encoder: Some(Encoder::H264X264),
            used_backend: Some(EncoderBackend::Cpu),
            fallback: None,
            fallback_occurred: false,
            actual_output_codec: Some(crate::config::OutputCodec::H264),
            actual_encoder_name: Some("libx264".to_string()),
            stats: ExecutionStats {
                encode_time_secs: 0.0,
                input_size: 0,
                output_size: 0,
                vmaf: None,
            },
        };
        let config_snapshot = config.read().await.clone();

        pipeline
            .handle_finalize_failure(
                job.id,
                FinalizeFailureContext {
                    plan: &plan,
                    metadata: &metadata,
                    execution_result: &result,
                    config_snapshot: &config_snapshot,
                    start_time: std::time::Instant::now(),
                    temp_output_path: &temp_output,
                },
                &crate::error::AlchemistError::Unknown("disk full".to_string()),
            )
            .await;

        let updated = db
            .get_job_by_id(job.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing failed job"))?;
        assert_eq!(updated.status, crate::db::JobState::Failed);
        assert!(!temp_output.exists());

        let logs = db.get_logs(10, 0).await?;
        assert!(
            logs.iter()
                .any(|entry| entry.message.contains("Finalization failed"))
        );

        let _ = std::fs::remove_dir_all(temp_root);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
