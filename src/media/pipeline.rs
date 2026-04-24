use crate::db::Job;
use crate::error::Result;
use crate::media::analyzer::FfmpegAnalyzer;
use crate::media::executor::FfmpegExecutor;
use crate::media::planner::BasicPlanner;
use crate::orchestrator::AsyncExecutionObserver;
use crate::orchestrator::Transcoder;
use crate::system::hardware::HardwareState;
use crate::telemetry::{TelemetryEvent, encoder_label, hardware_label, resolution_bucket};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::sync::{Mutex, RwLock};

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
    Bitrate { kbps: u32 },
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
    Mp3,
}

impl AudioCodec {
    pub fn ffmpeg_name(&self) -> &'static str {
        match self {
            Self::Aac => "aac",
            Self::Opus => "libopus",
            Self::Mp3 => "libmp3lame",
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
    Scale {
        width: u32,
        height: u32,
    },
    StripHdrMetadata,
    Custom {
        filter: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodePlan {
    pub decision: TranscodeDecision,
    pub is_remux: bool,
    pub copy_video: bool,
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
    event_channels: Arc<crate::db::EventChannels>,
    dry_run: bool,
}

struct FinalizeJobContext<'a> {
    output_path: &'a Path,
    temp_output_path: &'a Path,
    plan: &'a TranscodePlan,
    bypass_quality_gates: bool,
    start_time: std::time::Instant,
    encode_started_at: chrono::DateTime<chrono::Utc>,
    attempt_number: i32,
    metadata: &'a MediaMetadata,
    execution_result: &'a ExecutionResult,
}

struct FinalizeFailureContext<'a> {
    plan: &'a TranscodePlan,
    metadata: &'a MediaMetadata,
    execution_result: &'a ExecutionResult,
    config_snapshot: &'a crate::config::Config,
    start_time: std::time::Instant,
    encode_started_at: chrono::DateTime<chrono::Utc>,
    attempt_number: i32,
    temp_output_path: &'a Path,
}

const RESUME_STRATEGY_SEGMENT_V1: &str = "segment_v1";
const RESUME_SESSION_STATUS_ACTIVE: &str = "active";
const RESUME_SESSION_STATUS_SEGMENTS_COMPLETE: &str = "segments_complete";
const RESUME_SEGMENT_STATUS_PENDING: &str = "pending";
const RESUME_SEGMENT_STATUS_ENCODING: &str = "encoding";
const RESUME_SEGMENT_STATUS_COMPLETED: &str = "completed";
const RESUME_SEGMENT_LENGTH_SECS: i64 = 120;
#[cfg(test)]
static RESUME_SEGMENT_LENGTH_OVERRIDE: std::sync::OnceLock<std::sync::Mutex<Option<i64>>> =
    std::sync::OnceLock::new();

fn resume_segment_length_secs() -> i64 {
    #[cfg(test)]
    {
        let override_secs = RESUME_SEGMENT_LENGTH_OVERRIDE
            .get_or_init(|| std::sync::Mutex::new(None))
            .lock()
            .ok()
            .and_then(|guard| *guard);
        override_secs.unwrap_or(RESUME_SEGMENT_LENGTH_SECS)
    }

    #[cfg(not(test))]
    {
        RESUME_SEGMENT_LENGTH_SECS
    }
}

#[derive(Debug, Clone)]
struct ResumeSegment {
    segment_index: i64,
    start_secs: f64,
    duration_secs: f64,
    temp_path: PathBuf,
    status: String,
    attempt_count: i32,
}

struct ResumeSegmentObserver {
    job_id: i64,
    db: Arc<crate::db::Db>,
    event_channels: Arc<crate::db::EventChannels>,
    segment_start_secs: f64,
    segment_duration_secs: f64,
    total_duration_secs: f64,
    last_progress: Mutex<Option<(f64, Instant)>>,
}

impl ResumeSegmentObserver {
    fn new(
        job_id: i64,
        db: Arc<crate::db::Db>,
        event_channels: Arc<crate::db::EventChannels>,
        segment_start_secs: f64,
        segment_duration_secs: f64,
        total_duration_secs: f64,
    ) -> Self {
        Self {
            job_id,
            db,
            event_channels,
            segment_start_secs,
            segment_duration_secs,
            total_duration_secs,
            last_progress: Mutex::new(None),
        }
    }
}

impl AsyncExecutionObserver for ResumeSegmentObserver {
    async fn on_log(&self, message: String) {
        let _ = self.event_channels.jobs.send(crate::db::JobEvent::Log {
            level: "info".to_string(),
            job_id: Some(self.job_id),
            message: message.clone(),
        });
        if let Err(err) = self.db.add_log("info", Some(self.job_id), &message).await {
            tracing::warn!(
                job_id = self.job_id,
                "Failed to persist resume-segment log: {err}"
            );
        }
    }

    async fn on_progress(
        &self,
        progress: crate::media::ffmpeg::FFmpegProgress,
        total_duration: f64,
    ) {
        let segment_total = if total_duration > 0.0 {
            total_duration
        } else {
            self.segment_duration_secs
        };
        let segment_pct = progress.percentage(segment_total).clamp(0.0, 100.0);
        let completed_secs =
            self.segment_start_secs + (segment_pct / 100.0) * self.segment_duration_secs;
        let overall_pct = if self.total_duration_secs > 0.0 {
            (completed_secs / self.total_duration_secs * 100.0).clamp(0.0, 99.9)
        } else {
            0.0
        };
        let now = Instant::now();
        let mut last_progress = self.last_progress.lock().await;
        let should_persist = match *last_progress {
            Some((last_pct, last_time)) => {
                overall_pct >= last_pct + 0.5 || now.duration_since(last_time).as_secs() >= 2
            }
            None => true,
        };

        if should_persist {
            if let Err(err) = self.db.update_job_progress(self.job_id, overall_pct).await {
                tracing::warn!(
                    job_id = self.job_id,
                    "Failed to persist resume progress: {err}"
                );
            } else {
                *last_progress = Some((overall_pct, now));
            }
        }

        let _ = self
            .event_channels
            .jobs
            .send(crate::db::JobEvent::Progress {
                job_id: self.job_id,
                percentage: overall_pct,
                time: progress.time,
            });
    }
}

impl Pipeline {
    pub fn new(
        db: Arc<crate::db::Db>,
        orchestrator: Arc<Transcoder>,
        config: Arc<RwLock<crate::config::Config>>,
        hardware_state: HardwareState,
        event_channels: Arc<crate::db::EventChannels>,
        dry_run: bool,
    ) -> Self {
        Self {
            db,
            orchestrator,
            config,
            hardware_state,
            event_channels,
            dry_run,
        }
    }

    async fn store_job_input_metadata(&self, job_id: i64, metadata: &MediaMetadata) {
        if let Err(err) = self.db.set_job_input_metadata(job_id, metadata).await {
            tracing::warn!(job_id, "Failed to store input metadata: {err}");
        }
    }

    async fn record_job_log(&self, job_id: i64, level: &str, message: &str) {
        if let Err(err) = self.db.add_log(level, Some(job_id), message).await {
            tracing::warn!(job_id, "Failed to record log: {err}");
        }
    }

    async fn record_job_decision(&self, job_id: i64, action: &str, reason: &str) {
        if let Err(err) = self.db.add_decision(job_id, action, reason).await {
            tracing::warn!(job_id, "Failed to record decision: {err}");
        }
    }

    async fn record_job_decision_with_explanation(
        &self,
        job_id: i64,
        action: &str,
        explanation: &crate::explanations::Explanation,
    ) {
        if let Err(err) = self
            .db
            .add_decision_with_explanation(job_id, action, explanation)
            .await
        {
            tracing::warn!(job_id, "Failed to record decision explanation: {err}");
        }
    }

    async fn record_job_failure_explanation(
        &self,
        job_id: i64,
        explanation: &crate::explanations::Explanation,
    ) {
        if let Err(err) = self
            .db
            .upsert_job_failure_explanation(job_id, explanation)
            .await
        {
            tracing::warn!(job_id, "Failed to record failure explanation: {err}");
        }
    }

    async fn record_encode_attempt(&self, job_id: i64, input: crate::db::EncodeAttemptInput) {
        if let Err(err) = self.db.insert_encode_attempt(input).await {
            tracing::warn!(job_id, "Failed to record encode attempt: {err}");
        }
    }

    async fn purge_resume_session_state(&self, job_id: i64) -> Result<()> {
        let session = self.db.get_resume_session(job_id).await?;
        self.db.delete_resume_session(job_id).await?;
        if let Some(session) = session {
            let temp_dir = PathBuf::from(session.temp_dir);
            if temp_dir.exists() {
                tokio::fs::remove_dir_all(&temp_dir).await.map_err(|err| {
                    crate::error::AlchemistError::Io(std::io::Error::new(
                        err.kind(),
                        format!(
                            "Failed to remove resume temp dir {}: {err}",
                            temp_dir.display()
                        ),
                    ))
                })?;
            }
        }
        Ok(())
    }

    async fn prepare_resume_session(
        &self,
        job: &Job,
        plan: &TranscodePlan,
        metadata: &MediaMetadata,
        output_path: &Path,
    ) -> Result<Option<(crate::db::JobResumeSession, Vec<ResumeSegment>)>> {
        if !resumable_plan_supported(plan, metadata) {
            if self.db.get_resume_session(job.id).await?.is_some() {
                self.purge_resume_session_state(job.id).await?;
            }
            return Ok(None);
        }

        let mtime_hash = mtime_hash_from_path(Path::new(&job.input_path))?;
        let plan_hash = plan_hash_for_resume(plan, output_path, &mtime_hash)?;
        let temp_dir = resume_temp_dir_for(output_path, job.id);
        let concat_manifest_path = concat_manifest_path_for(&temp_dir);
        let existing_session = self.db.get_resume_session(job.id).await?;

        if let Some(session) = &existing_session {
            if session.strategy != RESUME_STRATEGY_SEGMENT_V1
                || session.plan_hash != plan_hash
                || session.mtime_hash != mtime_hash
            {
                self.purge_resume_session_state(job.id).await?;
            }
        }

        tokio::fs::create_dir_all(&temp_dir).await?;
        let session = self
            .db
            .upsert_resume_session(&crate::db::UpsertJobResumeSessionInput {
                job_id: job.id,
                strategy: RESUME_STRATEGY_SEGMENT_V1.to_string(),
                plan_hash,
                mtime_hash,
                temp_dir: temp_dir.display().to_string(),
                concat_manifest_path: concat_manifest_path.display().to_string(),
                segment_length_secs: resume_segment_length_secs(),
                status: RESUME_SESSION_STATUS_ACTIVE.to_string(),
            })
            .await?;

        let existing_segments = self.db.list_resume_segments(job.id).await?;
        let segments = enumerate_resume_segments(
            metadata.duration_secs,
            &temp_dir,
            output_path,
            &existing_segments,
        );
        for segment in &segments {
            self.db
                .upsert_resume_segment(&crate::db::UpsertJobResumeSegmentInput {
                    job_id: job.id,
                    segment_index: segment.segment_index,
                    start_secs: segment.start_secs,
                    duration_secs: segment.duration_secs,
                    temp_path: segment.temp_path.display().to_string(),
                    status: segment.status.clone(),
                    attempt_count: segment.attempt_count,
                })
                .await?;
        }

        Ok(Some((session, segments)))
    }

    async fn encode_resume_segment(
        &self,
        job: &Job,
        plan: &TranscodePlan,
        metadata: &MediaMetadata,
        segment: &ResumeSegment,
    ) -> Result<()> {
        let next_attempt = segment.attempt_count + 1;
        self.db
            .set_resume_segment_status(
                job.id,
                segment.segment_index,
                RESUME_SEGMENT_STATUS_ENCODING,
                next_attempt,
            )
            .await?;

        if segment.temp_path.exists() {
            let _ = tokio::fs::remove_file(&segment.temp_path).await;
        }

        let mut segment_plan = plan.clone();
        segment_plan.output_path = Some(segment.temp_path.clone());
        let hardware_info = self.hardware_state.snapshot().await;
        let observer: Arc<dyn crate::orchestrator::ExecutionObserver> =
            Arc::new(ResumeSegmentObserver::new(
                job.id,
                self.db.clone(),
                self.event_channels.clone(),
                segment.start_secs,
                segment.duration_secs,
                metadata.duration_secs,
            ));

        let result = self
            .orchestrator
            .transcode_media(crate::orchestrator::TranscodeRequest {
                job_id: Some(job.id),
                input: Path::new(&job.input_path),
                output: &segment.temp_path,
                hw_info: hardware_info.as_ref(),
                dry_run: self.dry_run,
                metadata,
                plan: &segment_plan,
                observer: Some(observer),
                clip_start_seconds: Some(segment.start_secs),
                clip_duration_seconds: Some(segment.duration_secs),
            })
            .await;

        match result {
            Ok(()) => {
                self.db
                    .set_resume_segment_status(
                        job.id,
                        segment.segment_index,
                        RESUME_SEGMENT_STATUS_COMPLETED,
                        next_attempt,
                    )
                    .await?;
                let completed = self.db.completed_resume_duration_secs(job.id).await?;
                let progress = if metadata.duration_secs > 0.0 {
                    (completed / metadata.duration_secs * 100.0).clamp(0.0, 99.9)
                } else {
                    0.0
                };
                self.update_job_progress(job.id, progress).await;
                Ok(())
            }
            Err(err) => {
                let _ = tokio::fs::remove_file(&segment.temp_path).await;
                self.db
                    .set_resume_segment_status(
                        job.id,
                        segment.segment_index,
                        RESUME_SEGMENT_STATUS_PENDING,
                        next_attempt,
                    )
                    .await?;
                Err(err)
            }
        }
    }

    async fn concat_resume_segments(
        &self,
        job_id: i64,
        session: &crate::db::JobResumeSession,
        segments: &[ResumeSegment],
        temp_output_path: &Path,
        container: &str,
    ) -> Result<()> {
        let manifest_path = PathBuf::from(&session.concat_manifest_path);
        let mut manifest = String::from("ffconcat version 1.0\n");
        for segment in segments {
            manifest.push_str("file '");
            manifest.push_str(&escape_ffconcat_path(&segment.temp_path));
            manifest.push_str("'\n");
        }
        tokio::fs::write(&manifest_path, manifest).await?;

        if temp_output_path.exists() {
            let _ = tokio::fs::remove_file(temp_output_path).await;
        }

        let output = tokio::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-y",
                "-loglevel",
                "error",
                "-f",
                "concat",
                "-safe",
                "0",
                "-i",
            ])
            .arg(&manifest_path)
            .args(["-c", "copy", "-f"])
            .arg(ffmpeg_muxer_for_container(container))
            .arg(temp_output_path)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let message = if stderr.is_empty() {
                "FFmpeg concat failed".to_string()
            } else {
                format!("FFmpeg concat failed: {stderr}")
            };
            self.record_job_log(job_id, "error", &message).await;
            return Err(crate::error::AlchemistError::FFmpeg(message));
        }

        Ok(())
    }

    async fn build_execution_result_for_output(
        &self,
        job_id: i64,
        plan: &TranscodePlan,
        output_path: &Path,
    ) -> ExecutionResult {
        let encoder = plan.encoder;
        let planned_output_codec = plan.output_codec.unwrap_or_else(|| {
            encoder
                .map(Encoder::output_codec)
                .unwrap_or(plan.requested_codec)
        });
        let actual_probe = if !self.dry_run && output_path.exists() {
            crate::media::analyzer::Analyzer::probe_output_details(output_path)
                .await
                .ok()
        } else {
            None
        };
        let actual_output_codec = actual_probe
            .as_ref()
            .and_then(|probe| crate::media::executor::output_codec_from_name(&probe.codec_name));
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
                    encoder.map(|enc| enc.ffmpeg_encoder_name().to_string())
                }
            });
        let codec_mismatch =
            actual_output_codec.is_some_and(|actual_codec| actual_codec != planned_output_codec);
        let encoder_mismatch = encoder.is_some_and(|enc| {
            actual_probe
                .as_ref()
                .and_then(|probe| probe.stream_encoder_tag.as_deref())
                .is_some_and(|tag| !crate::media::executor::encoder_tag_matches(enc, tag))
        });

        if let (true, Some(codec)) = (codec_mismatch, actual_output_codec) {
            tracing::warn!(
                "Job {}: Planned codec {} but resumable output probed as {}",
                job_id,
                planned_output_codec.as_str(),
                codec.as_str()
            );
        }

        ExecutionResult {
            requested_codec: plan.requested_codec,
            planned_output_codec,
            requested_encoder: encoder,
            used_encoder: encoder,
            used_backend: plan.backend.or_else(|| encoder.map(Encoder::backend)),
            fallback: plan.fallback.clone(),
            fallback_occurred: plan.fallback.is_some() || codec_mismatch || encoder_mismatch,
            actual_output_codec,
            actual_encoder_name,
        }
    }

    async fn execute_resumable_transcode(
        &self,
        job: &Job,
        plan: &TranscodePlan,
        metadata: &MediaMetadata,
        temp_output_path: &Path,
    ) -> Result<Option<ExecutionResult>> {
        let Some((session, segments)) = self
            .prepare_resume_session(job, plan, metadata, Path::new(&job.output_path))
            .await?
        else {
            return Ok(None);
        };

        let pending_segments = segments
            .iter()
            .filter(|segment| segment.status != RESUME_SEGMENT_STATUS_COMPLETED)
            .cloned()
            .collect::<Vec<_>>();
        let completed_secs = segments
            .iter()
            .filter(|segment| segment.status == RESUME_SEGMENT_STATUS_COMPLETED)
            .map(|segment| segment.duration_secs)
            .sum::<f64>();
        if metadata.duration_secs > 0.0 && completed_secs > 0.0 {
            let progress = (completed_secs / metadata.duration_secs * 100.0).clamp(0.0, 99.9);
            self.update_job_progress(job.id, progress).await;
        }

        for segment in &pending_segments {
            if self.should_stop_job(job.id).await? {
                return Err(crate::error::AlchemistError::Cancelled);
            }
            self.encode_resume_segment(job, plan, metadata, segment)
                .await?;
        }

        self.db
            .upsert_resume_session(&crate::db::UpsertJobResumeSessionInput {
                job_id: session.job_id,
                strategy: session.strategy.clone(),
                plan_hash: session.plan_hash.clone(),
                mtime_hash: session.mtime_hash.clone(),
                temp_dir: session.temp_dir.clone(),
                concat_manifest_path: session.concat_manifest_path.clone(),
                segment_length_secs: session.segment_length_secs,
                status: RESUME_SESSION_STATUS_SEGMENTS_COMPLETE.to_string(),
            })
            .await?;

        let completed_segments = self
            .db
            .list_resume_segments(job.id)
            .await?
            .into_iter()
            .map(|segment| ResumeSegment {
                segment_index: segment.segment_index,
                start_secs: segment.start_secs,
                duration_secs: segment.duration_secs,
                temp_path: PathBuf::from(segment.temp_path),
                status: segment.status,
                attempt_count: segment.attempt_count,
            })
            .collect::<Vec<_>>();

        self.concat_resume_segments(
            job.id,
            &session,
            &completed_segments,
            temp_output_path,
            &plan.container,
        )
        .await?;

        Ok(Some(
            self.build_execution_result_for_output(job.id, plan, temp_output_path)
                .await,
        ))
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

pub fn default_file_settings() -> crate::db::FileSettings {
    crate::db::FileSettings {
        id: 1,
        delete_source: false,
        output_extension: "mkv".to_string(),
        output_suffix: "-alchemist".to_string(),
        replace_strategy: "keep".to_string(),
        output_root: None,
    }
}

pub(crate) fn matches_generated_output_pattern(
    path: &Path,
    settings: &crate::db::FileSettings,
) -> bool {
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

pub async fn skip_reason_for_discovered_path(
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

fn resume_temp_dir_for(output_path: &Path, job_id: i64) -> PathBuf {
    let parent = output_path.parent().unwrap_or_else(|| Path::new(""));
    let filename = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    parent.join(format!(".{filename}.alchemist.resume-{job_id}"))
}

fn concat_manifest_path_for(temp_dir: &Path) -> PathBuf {
    temp_dir.join("segments.ffconcat")
}

fn escape_ffconcat_path(path: &Path) -> String {
    path.display().to_string().replace('\'', "'\\''")
}

fn ffmpeg_muxer_for_container(container: &str) -> String {
    match container.to_ascii_lowercase().as_str() {
        "mkv" => "matroska".to_string(),
        "mp4" => "mp4".to_string(),
        "mov" => "mov".to_string(),
        "avi" => "avi".to_string(),
        other => other.to_string(),
    }
}

fn mtime_hash_from_path(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    let duration = modified
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    Ok(format!(
        "{}.{:09}",
        duration.as_secs(),
        duration.subsec_nanos()
    ))
}

fn plan_hash_for_resume(
    plan: &TranscodePlan,
    output_path: &Path,
    mtime_hash: &str,
) -> Result<String> {
    let serialized = serde_json::to_vec(&(plan, output_path, mtime_hash)).map_err(|err| {
        crate::error::AlchemistError::Unknown(format!("Failed to hash plan: {err}"))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(serialized);
    Ok(format!("{:x}", hasher.finalize()))
}

fn resumable_plan_supported(plan: &TranscodePlan, metadata: &MediaMetadata) -> bool {
    matches!(plan.decision, TranscodeDecision::Transcode { .. })
        && !plan.is_remux
        && !matches!(plan.subtitles, SubtitleStreamPlan::Extract { .. })
        && metadata.duration_secs > 0.0
}

fn enumerate_resume_segments(
    total_duration_secs: f64,
    temp_dir: &Path,
    output_path: &Path,
    existing_segments: &[crate::db::JobResumeSegment],
) -> Vec<ResumeSegment> {
    let extension = output_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("mkv");
    let segment_length_secs = resume_segment_length_secs();
    let total_segments = (total_duration_secs / segment_length_secs as f64).ceil() as i64;
    let mut by_index = HashMap::new();
    for segment in existing_segments {
        by_index.insert(segment.segment_index, segment);
    }

    (0..total_segments)
        .map(|segment_index| {
            let start_secs = segment_index as f64 * segment_length_secs as f64;
            let duration_secs = (total_duration_secs - start_secs).min(segment_length_secs as f64);
            let temp_path = temp_dir.join(format!("segment-{segment_index:05}.{extension}"));
            let existing = by_index.get(&segment_index);
            let status = match existing {
                Some(segment)
                    if segment.status == RESUME_SEGMENT_STATUS_COMPLETED && temp_path.exists() =>
                {
                    segment.status.clone()
                }
                Some(segment) => {
                    if segment.status == RESUME_SEGMENT_STATUS_COMPLETED && !temp_path.exists() {
                        RESUME_SEGMENT_STATUS_PENDING.to_string()
                    } else {
                        segment.status.clone()
                    }
                }
                None => RESUME_SEGMENT_STATUS_PENDING.to_string(),
            };
            ResumeSegment {
                segment_index,
                start_secs,
                duration_secs,
                temp_path,
                status,
                attempt_count: existing.map(|segment| segment.attempt_count).unwrap_or(0),
            }
        })
        .collect()
}

impl Pipeline {
    /// Runs only the analysis and planning phases for a job.
    /// Does not execute any encode. Used by the startup
    /// auto-analyzer to populate skip/transcode decisions.
    pub async fn analyze_job_only(&self, job: crate::db::Job) -> Result<()> {
        let job_id = job.id;

        // Update status to analyzing
        self.update_job_state(job_id, crate::db::JobState::Analyzing)
            .await?;

        // Run ffprobe analysis
        let analyzer = crate::media::analyzer::FfmpegAnalyzer;
        let analysis = match analyzer
            .analyze(std::path::Path::new(&job.input_path))
            .await
        {
            Ok(a) => {
                // Store analyzed metadata for completed job detail retrieval
                self.store_job_input_metadata(job_id, &a.metadata).await;
                a
            }
            Err(e) => {
                let reason = format!("analysis_failed|error={e}");
                let failure_explanation = crate::explanations::failure_from_summary(&reason);
                self.record_job_log(job_id, "error", &reason).await;
                self.record_job_failure_explanation(job_id, &failure_explanation)
                    .await;
                self.update_job_state(job_id, crate::db::JobState::Failed)
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
                let reason = format!("profile_lookup_failed|error={e}");
                let failure_explanation = crate::explanations::failure_from_summary(&reason);
                self.record_job_log(job_id, "error", &reason).await;
                self.record_job_failure_explanation(job_id, &failure_explanation)
                    .await;
                self.update_job_state(job_id, crate::db::JobState::Failed)
                    .await?;
                return Ok(());
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
                let failure_explanation = crate::explanations::failure_from_summary(&reason);
                self.record_job_log(job_id, "error", &reason).await;
                self.record_job_failure_explanation(job_id, &failure_explanation)
                    .await;
                self.update_job_state(job_id, crate::db::JobState::Failed)
                    .await?;
                return Ok(());
            }
        };

        // Store the decision and return to queued — do NOT encode
        match &plan.decision {
            crate::media::pipeline::TranscodeDecision::Skip { reason } => {
                let skip_code = reason.split('|').next().unwrap_or(reason).trim();
                tracing::info!(
                    job_id = job_id,
                    skip_code = skip_code,
                    "Job skipped: {}",
                    skip_code
                );
                self.record_job_decision(job_id, "skip", reason).await;
                self.update_job_state(job_id, crate::db::JobState::Skipped)
                    .await?;
            }
            crate::media::pipeline::TranscodeDecision::Remux { reason } => {
                self.record_job_decision(job_id, "transcode", reason).await;
                // Leave as queued — will be picked up for remux when engine starts
                self.update_job_state(job_id, crate::db::JobState::Queued)
                    .await?;
            }
            crate::media::pipeline::TranscodeDecision::Transcode { reason } => {
                self.record_job_decision(job_id, "transcode", reason).await;
                // Leave as queued — will be picked up for encoding when engine starts
                self.update_job_state(job_id, crate::db::JobState::Queued)
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
            self.record_job_decision(job.id, "skip", "Output path matches input path")
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
            self.record_job_decision(job.id, "skip", "Output already exists")
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
        let current_attempt_number = job.attempt_count + 1;
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
                self.record_job_log(job.id, "error", &msg).await;
                let explanation = crate::explanations::failure_from_summary(&msg);
                self.record_job_failure_explanation(job.id, &explanation)
                    .await;
                if let Err(e) = self
                    .update_job_state(job.id, crate::db::JobState::Failed)
                    .await
                {
                    tracing::warn!(job_id = job.id, "Failed to update job state: {e}");
                }
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
        let conversion_job = match self.db.get_conversion_job_by_linked_job_id(job.id).await {
            Ok(conversion_job) => conversion_job,
            Err(err) => {
                let msg = format!("Failed to load linked conversion job: {err}");
                tracing::error!("Job {}: {}", job.id, msg);
                self.record_job_log(job.id, "error", &msg).await;
                let explanation = crate::explanations::failure_from_summary(&msg);
                self.record_job_failure_explanation(job.id, &explanation)
                    .await;
                if let Err(e) = self
                    .update_job_state(job.id, crate::db::JobState::Failed)
                    .await
                {
                    tracing::warn!(job_id = job.id, "Failed to update job state: {e}");
                }
                return Err(JobFailure::Transient);
            }
        };
        let bypass_quality_gates = conversion_job.is_some();
        let mut plan = if let Some(conversion_job) = conversion_job.as_ref() {
            let settings: crate::conversion::ConversionSettings =
                match serde_json::from_str(&conversion_job.settings_json) {
                    Ok(settings) => settings,
                    Err(err) => {
                        let msg = format!("Invalid conversion job settings: {err}");
                        tracing::error!("Job {}: {}", job.id, msg);
                        self.record_job_log(job.id, "error", &msg).await;
                        let explanation = crate::explanations::failure_from_summary(&msg);
                        self.record_job_failure_explanation(job.id, &explanation)
                            .await;
                        if let Err(e) = self
                            .update_job_state(job.id, crate::db::JobState::Failed)
                            .await
                        {
                            tracing::warn!(job_id = job.id, "Failed to update job state: {e}");
                        }
                        return Err(JobFailure::PlannerBug);
                    }
                };
            match crate::conversion::build_plan(&analysis, &output_path, &settings, hw_info.clone())
            {
                Ok(plan) => plan,
                Err(err) => {
                    let msg = format!("Conversion planning failed: {err}");
                    tracing::error!("Job {}: {}", job.id, msg);
                    self.record_job_log(job.id, "error", &msg).await;
                    let explanation = crate::explanations::failure_from_summary(&msg);
                    self.record_job_failure_explanation(job.id, &explanation)
                        .await;
                    if let Err(e) = self
                        .update_job_state(job.id, crate::db::JobState::Failed)
                        .await
                    {
                        tracing::warn!(job_id = job.id, "Failed to update job state: {e}");
                    }
                    return Err(JobFailure::PlannerBug);
                }
            }
        } else {
            let planner = BasicPlanner::new(Arc::new(config_snapshot.clone()), hw_info.clone());
            let profile = match self.db.get_profile_for_path(&job.input_path).await {
                Ok(profile) => profile,
                Err(err) => {
                    let msg = format!("Failed to resolve library profile: {err}");
                    tracing::error!("Job {}: {}", job.id, msg);
                    self.record_job_log(job.id, "error", &msg).await;
                    let explanation = crate::explanations::failure_from_summary(&msg);
                    self.record_job_failure_explanation(job.id, &explanation)
                        .await;
                    if let Err(e) = self
                        .update_job_state(job.id, crate::db::JobState::Failed)
                        .await
                    {
                        tracing::warn!(job_id = job.id, "Failed to update job state: {e}");
                    }
                    return Err(JobFailure::Transient);
                }
            };
            match planner
                .plan(&analysis, &output_path, profile.as_ref())
                .await
            {
                Ok(plan) => plan,
                Err(e) => {
                    let msg = format!("Planner failed: {e}");
                    tracing::error!("Job {}: {}", job.id, msg);
                    self.record_job_log(job.id, "error", &msg).await;
                    let explanation = crate::explanations::failure_from_summary(&msg);
                    self.record_job_failure_explanation(job.id, &explanation)
                        .await;
                    if let Err(e) = self
                        .update_job_state(job.id, crate::db::JobState::Failed)
                        .await
                    {
                        tracing::warn!(job_id = job.id, "Failed to update job state: {e}");
                    }
                    return Err(JobFailure::PlannerBug);
                }
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

        match self.should_stop_job(job.id).await {
            Ok(true) => {
                tracing::info!("Job {} was cancelled during encode planning.", job.id);
                return Ok(());
            }
            Ok(false) => {}
            Err(_) => return Err(JobFailure::Transient),
        }

        let (should_execute, action, reason, next_status) = match &plan.decision {
            TranscodeDecision::Transcode { reason } => (
                true,
                "encode",
                reason.clone(),
                crate::db::JobState::Encoding,
            ),
            TranscodeDecision::Remux { reason } => {
                tracing::info!(
                    "Job {}: Remuxing MP4→MKV (stream copy, no re-encode)",
                    job.id
                );
                (true, "remux", reason.clone(), crate::db::JobState::Remuxing)
            }
            TranscodeDecision::Skip { reason } => {
                (false, "skip", reason.clone(), crate::db::JobState::Skipped)
            }
        };

        if !should_execute {
            let explanation = crate::explanations::decision_from_legacy("skip", &reason);
            tracing::info!(
                "Decision: SKIP Job {} - {} (code={}, summary={})",
                job.id,
                &reason,
                explanation.code,
                explanation.summary
            );
            self.record_job_decision(job.id, "skip", &reason).await;
            if let Err(e) = self
                .update_job_state(job.id, crate::db::JobState::Skipped)
                .await
            {
                tracing::warn!(job_id = job.id, "Failed to update job state: {e}");
            }
            return Ok(());
        }

        tracing::info!(
            "Decision: {} Job {} - {}",
            action.to_ascii_uppercase(),
            job.id,
            &reason
        );
        let explanation = crate::explanations::decision_from_legacy(action, &reason);
        self.record_job_decision_with_explanation(job.id, action, &explanation)
            .await;
        let _ = self
            .event_channels
            .jobs
            .send(crate::db::JobEvent::Decision {
                job_id: job.id,
                action: action.to_string(),
                reason: explanation.legacy_reason.clone(),
                explanation: Some(explanation.clone()),
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
            self.event_channels.clone(),
            self.dry_run,
        );

        let encode_started_at = chrono::Utc::now();
        let execution_result = match self
            .execute_resumable_transcode(&job, &plan, metadata, &temp_output_path)
            .await
        {
            Ok(Some(result)) => Ok(result),
            Ok(None) => executor.execute(&job, &plan, &analysis).await,
            Err(err) => Err(err),
        };
        match execution_result {
            Ok(result) => {
                if result.fallback_occurred && !plan.allow_fallback {
                    tracing::error!("Job {}: Encoder fallback detected and not allowed.", job.id);
                    let summary = "Encoder fallback detected and not allowed.";
                    let explanation = crate::explanations::failure_from_summary(summary);
                    self.record_job_log(job.id, "error", summary).await;
                    self.record_job_failure_explanation(job.id, &explanation)
                        .await;
                    if let Err(e) = self
                        .update_job_state(job.id, crate::db::JobState::Failed)
                        .await
                    {
                        tracing::warn!(job_id = job.id, "Failed to update job state: {e}");
                    }
                    self.record_encode_attempt(
                        job.id,
                        crate::db::EncodeAttemptInput {
                            job_id: job.id,
                            attempt_number: current_attempt_number,
                            started_at: Some(encode_started_at.to_rfc3339()),
                            outcome: "failed".to_string(),
                            failure_code: Some("fallback_blocked".to_string()),
                            failure_summary: Some(summary.to_string()),
                            input_size_bytes: Some(metadata.size_bytes as i64),
                            output_size_bytes: None,
                            encode_time_seconds: Some(start_time.elapsed().as_secs_f64()),
                        },
                    )
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
                            bypass_quality_gates,
                            start_time,
                            encode_started_at,
                            attempt_number: current_attempt_number,
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
                            encode_started_at,
                            attempt_number: current_attempt_number,
                            temp_output_path: &temp_output_path,
                        },
                        &err,
                    )
                    .await;
                    return Err(JobFailure::Transient);
                }

                if self
                    .db
                    .get_resume_session(job.id)
                    .await
                    .ok()
                    .flatten()
                    .is_some()
                {
                    if let Err(err) = self.purge_resume_session_state(job.id).await {
                        tracing::warn!(
                            job_id = job.id,
                            "Failed to purge resume session after successful finalize: {err}"
                        );
                    }
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
                    if let Err(e) = self
                        .update_job_state(job.id, crate::db::JobState::Cancelled)
                        .await
                    {
                        tracing::warn!(
                            job_id = job.id,
                            "Failed to update job state to cancelled: {e}"
                        );
                    }
                    self.record_encode_attempt(
                        job.id,
                        crate::db::EncodeAttemptInput {
                            job_id: job.id,
                            attempt_number: current_attempt_number,
                            started_at: Some(encode_started_at.to_rfc3339()),
                            outcome: "cancelled".to_string(),
                            failure_code: None,
                            failure_summary: None,
                            input_size_bytes: Some(metadata.size_bytes as i64),
                            output_size_bytes: None,
                            encode_time_seconds: Some(start_time.elapsed().as_secs_f64()),
                        },
                    )
                    .await;
                } else {
                    let msg = format!("Transcode failed: {e}");
                    tracing::error!("Job {}: {}", job.id, msg);
                    self.record_job_log(job.id, "error", &msg).await;
                    let explanation = crate::explanations::failure_from_summary(&msg);
                    self.record_job_failure_explanation(job.id, &explanation)
                        .await;
                    if let Err(e) = self
                        .update_job_state(job.id, crate::db::JobState::Failed)
                        .await
                    {
                        tracing::warn!(
                            job_id = job.id,
                            "Failed to update job state to failed: {e}"
                        );
                    }
                    self.record_encode_attempt(
                        job.id,
                        crate::db::EncodeAttemptInput {
                            job_id: job.id,
                            attempt_number: current_attempt_number,
                            started_at: Some(encode_started_at.to_rfc3339()),
                            outcome: "failed".to_string(),
                            failure_code: Some(explanation.code.clone()),
                            failure_summary: Some(msg),
                            input_size_bytes: Some(metadata.size_bytes as i64),
                            output_size_bytes: None,
                            encode_time_seconds: Some(start_time.elapsed().as_secs_f64()),
                        },
                    )
                    .await;
                }
                Err(map_failure(&e))
            }
        }
    }

    async fn update_job_state(&self, job_id: i64, status: crate::db::JobState) -> Result<()> {
        if self.orchestrator.is_cancel_requested(job_id).await {
            match status {
                crate::db::JobState::Encoding
                | crate::db::JobState::Remuxing
                | crate::db::JobState::Skipped
                | crate::db::JobState::Completed => {
                    tracing::info!(
                        "Ignoring state update to {:?} for job {} because it was cancelled",
                        status,
                        job_id
                    );
                    self.orchestrator.remove_cancel_request(job_id).await;
                    return Ok(());
                }
                _ => {}
            }
        }

        if let Err(e) = self.db.update_job_status(job_id, status).await {
            tracing::error!("Failed to update job {} status {:?}: {}", job_id, status, e);
            return Err(e);
        }

        // Remove from cancel_requested if it's a terminal state
        match status {
            crate::db::JobState::Completed
            | crate::db::JobState::Failed
            | crate::db::JobState::Cancelled
            | crate::db::JobState::Skipped => {
                self.orchestrator.remove_cancel_request(job_id).await;
            }
            _ => {}
        }

        let _ = self
            .event_channels
            .jobs
            .send(crate::db::JobEvent::StateChanged { job_id, status });
        Ok(())
    }

    async fn update_job_progress(&self, job_id: i64, progress: f64) {
        if let Err(e) = self.db.update_job_progress(job_id, progress).await {
            tracing::error!("Failed to update job progress: {}", e);
        }
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
            return Err(crate::error::AlchemistError::FFmpeg(
                "Input file is empty".to_string(),
            ));
        }

        let reduction = 1.0 - (output_size as f64 / input_size as f64);
        let encode_duration = context.start_time.elapsed().as_secs_f64();

        let config = self.config.read().await;
        let telemetry_enabled = config.system.enable_telemetry;

        if !context.bypass_quality_gates
            && (output_size == 0
                || (!context.plan.is_remux
                    && reduction < config.transcode.size_reduction_threshold))
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
            self.record_job_decision(job_id, "skip", &reason).await;
            self.update_job_state(job_id, crate::db::JobState::Skipped)
                .await?;
            return Ok(());
        }

        let mut vmaf_score = None;
        if !context.bypass_quality_gates && !context.plan.is_remux && config.quality.enable_vmaf {
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
                            self.record_job_decision(
                                job_id,
                                "skip",
                                &format!(
                                    "quality_below_threshold|metric=vmaf,score={:.1},threshold={:.1}",
                                    s, config.quality.min_vmaf_score
                                ),
                            )
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
            let reprobe_path = if context.temp_output_path.exists() {
                context.temp_output_path
            } else {
                context.output_path
            };
            match crate::media::analyzer::Analyzer::probe_async(reprobe_path).await {
                Ok(meta) => {
                    media_duration = meta.format.duration.parse::<f64>().unwrap_or(0.0);
                }
                Err(e) => {
                    tracing::warn!(
                        job_id,
                        path = %reprobe_path.display(),
                        "Failed to reprobe encoded output for duration: {e}"
                    );
                }
            }
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
        self.record_encode_attempt(
            job_id,
            crate::db::EncodeAttemptInput {
                job_id,
                attempt_number: context.attempt_number,
                started_at: Some(context.encode_started_at.to_rfc3339()),
                outcome: "completed".to_string(),
                failure_code: None,
                failure_summary: None,
                input_size_bytes: Some(input_size as i64),
                output_size_bytes: Some(output_size as i64),
                encode_time_seconds: Some(encode_duration),
            },
        )
        .await;

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
                            job_id,
                            context.output_path
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "Job {}: Cannot verify output {:?} after promotion ({}). Source preserved to prevent data loss",
                            job_id,
                            context.output_path,
                            e
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
        self.record_job_log(job_id, "error", &message).await;
        let failure_explanation = crate::explanations::failure_from_summary(&message);
        self.record_job_failure_explanation(job_id, &failure_explanation)
            .await;
        if let crate::error::AlchemistError::QualityCheckFailed(reason) = err {
            self.record_job_decision(job_id, "reject", reason).await;
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
        self.record_encode_attempt(
            job_id,
            crate::db::EncodeAttemptInput {
                job_id,
                attempt_number: context.attempt_number,
                started_at: Some(context.encode_started_at.to_rfc3339()),
                outcome: "failed".to_string(),
                failure_code: Some(failure_explanation.code.clone()),
                failure_summary: Some(message),
                input_size_bytes: Some(context.metadata.size_bytes as i64),
                output_size_bytes: None,
                encode_time_seconds: Some(context.start_time.elapsed().as_secs_f64()),
            },
        )
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
    use std::process::Command;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn ffmpeg_ready() -> bool {
        let ffmpeg = Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        let ffprobe = Command::new("ffprobe")
            .arg("-version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        ffmpeg && ffprobe
    }

    fn set_test_resume_segment_length(value: Option<i64>) {
        let lock = RESUME_SEGMENT_LENGTH_OVERRIDE.get_or_init(|| std::sync::Mutex::new(None));
        if let Ok(mut guard) = lock.lock() {
            *guard = value;
        }
    }

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
            copy_video: false,
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
        let (jobs_tx, _) = tokio::sync::broadcast::channel(100);
        let (config_tx, _) = tokio::sync::broadcast::channel(10);
        let (system_tx, _) = tokio::sync::broadcast::channel(10);
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
            event_channels,
            true,
        );

        let plan = TranscodePlan {
            decision: TranscodeDecision::Transcode {
                reason: "test".to_string(),
            },
            is_remux: false,
            copy_video: false,
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
                    encode_started_at: chrono::Utc::now(),
                    attempt_number: 1,
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

    #[tokio::test]
    async fn process_job_skips_even_when_decision_persistence_fails() -> anyhow::Result<()> {
        let db_path = std::env::temp_dir().join(format!(
            "alchemist_decision_persistence_{}.db",
            rand::random::<u64>()
        ));
        let temp_root = std::env::temp_dir().join(format!(
            "alchemist_decision_persistence_{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&temp_root)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        db.update_file_settings(false, "mkv", "-alchemist", "keep", None)
            .await?;

        let input = temp_root.join("movie.mkv");
        let output = temp_root.join("movie-alchemist.mkv");
        std::fs::write(&input, b"source")?;
        std::fs::write(&output, b"existing-output")?;

        let _ = db
            .enqueue_job(&input, &output, SystemTime::UNIX_EPOCH)
            .await?;
        let job = db
            .get_job_by_input_path(input.to_string_lossy().as_ref())
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing queued job"))?;

        sqlx::query(
            "CREATE TRIGGER decisions_fail_insert
             BEFORE INSERT ON decisions
             BEGIN
                 SELECT RAISE(FAIL, 'forced decision failure');
             END;",
        )
        .execute(&db.pool)
        .await?;

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
        let (jobs_tx, _) = tokio::sync::broadcast::channel(100);
        let (config_tx, _) = tokio::sync::broadcast::channel(10);
        let (system_tx, _) = tokio::sync::broadcast::channel(10);
        let event_channels = Arc::new(crate::db::EventChannels {
            jobs: jobs_tx,
            config: config_tx,
            system: system_tx,
        });
        let pipeline = Pipeline::new(
            db.clone(),
            Arc::new(Transcoder::new()),
            config,
            hardware_state,
            event_channels,
            false,
        );

        pipeline
            .process_job(job.clone())
            .await
            .map_err(|err| anyhow::anyhow!("process_job failed: {err:?}"))?;
        let updated = db
            .get_job_by_id(job.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing skipped job"))?;
        assert_eq!(updated.status, crate::db::JobState::Skipped);

        let _ = std::fs::remove_dir_all(temp_root);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn analyze_job_only_marks_job_failed_without_decision_on_profile_lookup_failure()
    -> anyhow::Result<()> {
        if !ffmpeg_ready() {
            return Ok(());
        }

        let db_path = std::env::temp_dir().join(format!(
            "alchemist_analyze_profile_failure_{}.db",
            rand::random::<u64>()
        ));
        let temp_root = std::env::temp_dir().join(format!(
            "alchemist_analyze_profile_failure_{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&temp_root)?;

        let input = temp_root.join("movie.mkv");
        let output = temp_root.join("movie-alchemist.mkv");
        let ffmpeg_status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=16x16:d=1",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&input)
            .status()?;
        if !ffmpeg_status.success() {
            return Err(anyhow::anyhow!(
                "ffmpeg failed to create analyze-only test input"
            ));
        }

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let _ = db
            .enqueue_job(&input, &output, SystemTime::UNIX_EPOCH)
            .await?;
        let job = db
            .get_job_by_input_path(input.to_string_lossy().as_ref())
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing queued job"))?;
        sqlx::query("DROP TABLE watch_dirs")
            .execute(&db.pool)
            .await?;

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
        let (jobs_tx, _) = tokio::sync::broadcast::channel(100);
        let (config_tx, _) = tokio::sync::broadcast::channel(10);
        let (system_tx, _) = tokio::sync::broadcast::channel(10);
        let event_channels = Arc::new(crate::db::EventChannels {
            jobs: jobs_tx,
            config: config_tx,
            system: system_tx,
        });
        let pipeline = Pipeline::new(
            db.clone(),
            Arc::new(Transcoder::new()),
            config,
            hardware_state,
            event_channels,
            false,
        );

        pipeline.analyze_job_only(job.clone()).await?;

        let updated = db
            .get_job_by_id(job.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing failed job"))?;
        assert_eq!(updated.status, crate::db::JobState::Failed);
        assert!(db.get_job_decision(job.id).await?.is_none());
        assert!(db.get_job_failure_explanation(job.id).await?.is_some());

        let _ = std::fs::remove_dir_all(temp_root);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn finalize_job_succeeds_when_encode_attempt_persistence_fails() -> anyhow::Result<()> {
        if !ffmpeg_ready() {
            return Ok(());
        }

        let db_path = std::env::temp_dir().join(format!(
            "alchemist_attempt_persistence_{}.db",
            rand::random::<u64>()
        ));
        let temp_root = std::env::temp_dir().join(format!(
            "alchemist_attempt_persistence_{}",
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

        let temp_output = temp_output_path_for(&output);
        let ffmpeg_status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=16x16:d=1",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-f",
                "matroska",
            ])
            .arg(&temp_output)
            .status()?;
        if !ffmpeg_status.success() {
            return Err(anyhow::anyhow!(
                "ffmpeg failed to generate finalize fixture"
            ));
        }

        sqlx::query("DROP TABLE encode_attempts")
            .execute(&db.pool)
            .await?;

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
        let (jobs_tx, _) = tokio::sync::broadcast::channel(100);
        let (config_tx, _) = tokio::sync::broadcast::channel(10);
        let (system_tx, _) = tokio::sync::broadcast::channel(10);
        let event_channels = Arc::new(crate::db::EventChannels {
            jobs: jobs_tx,
            config: config_tx,
            system: system_tx,
        });
        let pipeline = Pipeline::new(
            db.clone(),
            Arc::new(Transcoder::new()),
            config,
            hardware_state,
            event_channels,
            false,
        );

        let plan = TranscodePlan {
            decision: TranscodeDecision::Transcode {
                reason: "test".to_string(),
            },
            is_remux: false,
            copy_video: false,
            output_path: Some(temp_output.clone()),
            container: "mkv".to_string(),
            requested_codec: crate::config::OutputCodec::H264,
            output_codec: Some(crate::config::OutputCodec::H264),
            encoder: Some(Encoder::H264X264),
            backend: Some(EncoderBackend::Cpu),
            rate_control: Some(RateControl::Crf { value: 21 }),
            encoder_preset: Some("medium".to_string()),
            threads: 0,
            audio: AudioStreamPlan::Drop,
            audio_stream_indices: None,
            subtitles: SubtitleStreamPlan::Drop,
            filters: Vec::new(),
            allow_fallback: true,
            fallback: None,
        };
        let metadata = MediaMetadata {
            path: input.clone(),
            duration_secs: 1.0,
            codec_name: "h264".to_string(),
            width: 16,
            height: 16,
            bit_depth: Some(8),
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            size_bytes: 6,
            video_bitrate_bps: Some(10_000),
            container_bitrate_bps: Some(10_000),
            fps: 1.0,
            container: "mkv".to_string(),
            audio_codec: None,
            audio_bitrate_bps: None,
            audio_channels: None,
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
        };

        pipeline
            .finalize_job(
                job.clone(),
                &input,
                FinalizeJobContext {
                    output_path: &output,
                    temp_output_path: &temp_output,
                    plan: &plan,
                    bypass_quality_gates: true,
                    start_time: std::time::Instant::now(),
                    encode_started_at: chrono::Utc::now(),
                    attempt_number: 1,
                    metadata: &metadata,
                    execution_result: &result,
                },
            )
            .await?;

        let updated = db
            .get_job_by_id(job.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing completed job"))?;
        assert_eq!(updated.status, crate::db::JobState::Completed);

        let _ = std::fs::remove_dir_all(temp_root);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn finalize_failure_marks_failed_when_log_persistence_fails() -> anyhow::Result<()> {
        let db_path = std::env::temp_dir().join(format!(
            "alchemist_log_persistence_{}.db",
            rand::random::<u64>()
        ));
        let temp_root = std::env::temp_dir().join(format!(
            "alchemist_log_persistence_{}",
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
        sqlx::query("DROP TABLE logs").execute(&db.pool).await?;

        let config = Arc::new(RwLock::new(crate::config::Config::default()));
        let config_snapshot = config.read().await.clone();
        let hardware_state = HardwareState::new(Some(HardwareInfo {
            vendor: Vendor::Cpu,
            device_path: None,
            supported_codecs: vec!["av1".to_string(), "hevc".to_string(), "h264".to_string()],
            backends: Vec::new(),
            detection_notes: Vec::new(),
            selection_reason: String::new(),
            probe_summary: crate::system::hardware::ProbeSummary::default(),
        }));
        let (jobs_tx, _) = tokio::sync::broadcast::channel(100);
        let (config_tx, _) = tokio::sync::broadcast::channel(10);
        let (system_tx, _) = tokio::sync::broadcast::channel(10);
        let event_channels = Arc::new(crate::db::EventChannels {
            jobs: jobs_tx,
            config: config_tx,
            system: system_tx,
        });
        let pipeline = Pipeline::new(
            db.clone(),
            Arc::new(Transcoder::new()),
            config,
            hardware_state,
            event_channels,
            false,
        );

        let plan = TranscodePlan {
            decision: TranscodeDecision::Transcode {
                reason: "test".to_string(),
            },
            is_remux: false,
            copy_video: false,
            output_path: Some(temp_output.clone()),
            container: "mkv".to_string(),
            requested_codec: crate::config::OutputCodec::H264,
            output_codec: Some(crate::config::OutputCodec::H264),
            encoder: Some(Encoder::H264X264),
            backend: Some(EncoderBackend::Cpu),
            rate_control: Some(RateControl::Crf { value: 21 }),
            encoder_preset: Some("medium".to_string()),
            threads: 0,
            audio: AudioStreamPlan::Drop,
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
            width: 16,
            height: 16,
            bit_depth: Some(8),
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            size_bytes: 6,
            video_bitrate_bps: Some(10_000),
            container_bitrate_bps: Some(10_000),
            fps: 1.0,
            container: "mkv".to_string(),
            audio_codec: None,
            audio_bitrate_bps: None,
            audio_channels: None,
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
        };

        pipeline
            .handle_finalize_failure(
                job.id,
                FinalizeFailureContext {
                    plan: &plan,
                    metadata: &metadata,
                    execution_result: &result,
                    config_snapshot: &config_snapshot,
                    start_time: std::time::Instant::now(),
                    encode_started_at: chrono::Utc::now(),
                    attempt_number: 1,
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

        let _ = std::fs::remove_dir_all(temp_root);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn finalize_job_reprobes_encoded_output_duration_for_stats() -> anyhow::Result<()> {
        let ffmpeg_available = std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        let ffprobe_available = std::process::Command::new("ffprobe")
            .arg("-version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        if !ffmpeg_available || !ffprobe_available {
            return Ok(());
        }

        let db_path = std::env::temp_dir().join(format!(
            "alchemist_finalize_duration_{}.db",
            rand::random::<u64>()
        ));
        let temp_root = std::env::temp_dir().join(format!(
            "alchemist_finalize_duration_{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&temp_root)?;

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let input = temp_root.join("source.mkv");
        let output = temp_root.join("source-alchemist.mkv");
        std::fs::write(&input, b"source-bytes")?;

        let _ = db
            .enqueue_job(&input, &output, SystemTime::UNIX_EPOCH)
            .await?;
        let job = db
            .get_job_by_input_path(input.to_string_lossy().as_ref())
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing queued job"))?;
        let job_id = job.id;
        db.update_job_status(job.id, crate::db::JobState::Encoding)
            .await?;

        let temp_output = temp_output_path_for(&output);
        let ffmpeg_status = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=16x16:d=1",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-f",
                "matroska",
            ])
            .arg(&temp_output)
            .status()?;
        if !ffmpeg_status.success() {
            return Err(anyhow::anyhow!("ffmpeg failed to generate test output"));
        }

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
        let (jobs_tx, _) = tokio::sync::broadcast::channel(100);
        let (config_tx, _) = tokio::sync::broadcast::channel(10);
        let (system_tx, _) = tokio::sync::broadcast::channel(10);
        let event_channels = Arc::new(crate::db::EventChannels {
            jobs: jobs_tx,
            config: config_tx,
            system: system_tx,
        });
        let pipeline = Pipeline::new(
            db.clone(),
            Arc::new(Transcoder::new()),
            config,
            hardware_state,
            event_channels,
            true,
        );

        let plan = TranscodePlan {
            decision: TranscodeDecision::Transcode {
                reason: "test".to_string(),
            },
            is_remux: false,
            copy_video: false,
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
            duration_secs: 0.0,
            codec_name: "unknown".to_string(),
            width: 16,
            height: 16,
            bit_depth: Some(8),
            color_primaries: None,
            color_transfer: None,
            color_space: None,
            color_range: None,
            size_bytes: 12,
            video_bitrate_bps: None,
            container_bitrate_bps: None,
            fps: 24.0,
            container: "mkv".to_string(),
            audio_codec: None,
            audio_bitrate_bps: None,
            audio_channels: None,
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
        };

        pipeline
            .finalize_job(
                job,
                &input,
                FinalizeJobContext {
                    output_path: &output,
                    temp_output_path: &temp_output,
                    plan: &plan,
                    bypass_quality_gates: true,
                    start_time: std::time::Instant::now(),
                    encode_started_at: chrono::Utc::now(),
                    attempt_number: 1,
                    metadata: &metadata,
                    execution_result: &result,
                },
            )
            .await?;

        let stats = db.get_encode_stats_by_job_id(job_id).await?;
        assert!(stats.encode_speed > 0.0);
        assert!(stats.avg_bitrate_kbps > 0.0);
        assert!(output.exists());
        assert!(!temp_output.exists());

        db.pool.close().await;
        let _ = std::fs::remove_dir_all(temp_root);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn resumable_transcode_skips_completed_segments_on_retry() -> anyhow::Result<()> {
        if !ffmpeg_ready() {
            return Ok(());
        }
        set_test_resume_segment_length(Some(1));

        let db_path = std::env::temp_dir().join(format!(
            "alchemist_resume_retry_{}.db",
            rand::random::<u64>()
        ));
        let temp_root =
            std::env::temp_dir().join(format!("alchemist_resume_retry_{}", rand::random::<u64>()));
        std::fs::create_dir_all(&temp_root)?;

        let input = temp_root.join("resume-source.mkv");
        let output = temp_root.join("resume-output.mkv");
        let ffmpeg_status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=16x16:r=1:d=3",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&input)
            .status()?;
        if !ffmpeg_status.success() {
            return Err(anyhow::anyhow!("ffmpeg failed to create resume test input"));
        }

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let _ = db
            .enqueue_job(&input, &output, SystemTime::UNIX_EPOCH)
            .await?;
        let job = db
            .get_job_by_input_path(input.to_string_lossy().as_ref())
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing queued job"))?;

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
        let (jobs_tx, _) = tokio::sync::broadcast::channel(100);
        let (config_tx, _) = tokio::sync::broadcast::channel(10);
        let (system_tx, _) = tokio::sync::broadcast::channel(10);
        let event_channels = Arc::new(crate::db::EventChannels {
            jobs: jobs_tx,
            config: config_tx,
            system: system_tx,
        });
        let pipeline = Pipeline::new(
            db.clone(),
            Arc::new(Transcoder::new()),
            config,
            hardware_state,
            event_channels,
            false,
        );

        let analyzer = FfmpegAnalyzer;
        let analysis = analyzer.analyze(&input).await?;
        let plan = TranscodePlan {
            decision: TranscodeDecision::Transcode {
                reason: "resume-test".to_string(),
            },
            is_remux: false,
            copy_video: false,
            output_path: Some(temp_output_path_for(&output)),
            container: "mkv".to_string(),
            requested_codec: crate::config::OutputCodec::H264,
            output_codec: Some(crate::config::OutputCodec::H264),
            encoder: Some(Encoder::H264X264),
            backend: Some(EncoderBackend::Cpu),
            rate_control: Some(RateControl::Crf { value: 21 }),
            encoder_preset: Some("ultrafast".to_string()),
            threads: 0,
            audio: AudioStreamPlan::Drop,
            audio_stream_indices: None,
            subtitles: SubtitleStreamPlan::Drop,
            filters: Vec::new(),
            allow_fallback: true,
            fallback: None,
        };

        let (_session, segments) = pipeline
            .prepare_resume_session(&job, &plan, &analysis.metadata, &output)
            .await?
            .ok_or_else(|| anyhow::anyhow!("resume session not created"))?;
        pipeline
            .encode_resume_segment(&job, &plan, &analysis.metadata, &segments[0])
            .await?;
        let first_segment_mtime = std::fs::metadata(&segments[0].temp_path)?.modified()?;

        let temp_output = temp_output_path_for(&output);
        let result = pipeline
            .execute_resumable_transcode(&job, &plan, &analysis.metadata, &temp_output)
            .await?;
        assert!(result.is_some());
        assert!(temp_output.exists());
        assert_eq!(
            std::fs::metadata(&segments[0].temp_path)?.modified()?,
            first_segment_mtime
        );

        let segments = db.list_resume_segments(job.id).await?;
        assert_eq!(segments.len(), 3);
        assert!(segments.iter().all(|segment| segment.status == "completed"));

        let _ = pipeline.purge_resume_session_state(job.id).await;
        let _ = std::fs::remove_dir_all(temp_root);
        let _ = std::fs::remove_file(db_path);
        set_test_resume_segment_length(None);
        Ok(())
    }

    #[tokio::test]
    async fn resumable_transcode_invalidates_stale_session_when_input_changes() -> anyhow::Result<()>
    {
        if !ffmpeg_ready() {
            return Ok(());
        }
        set_test_resume_segment_length(Some(1));

        let db_path = std::env::temp_dir().join(format!(
            "alchemist_resume_invalidate_{}.db",
            rand::random::<u64>()
        ));
        let temp_root = std::env::temp_dir().join(format!(
            "alchemist_resume_invalidate_{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&temp_root)?;

        let input = temp_root.join("invalidate-source.mkv");
        let output = temp_root.join("invalidate-output.mkv");
        let ffmpeg_status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=16x16:r=1:d=3",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&input)
            .status()?;
        if !ffmpeg_status.success() {
            return Err(anyhow::anyhow!(
                "ffmpeg failed to create invalidation test input"
            ));
        }

        let db = Arc::new(Db::new(db_path.to_string_lossy().as_ref()).await?);
        let _ = db
            .enqueue_job(&input, &output, SystemTime::UNIX_EPOCH)
            .await?;
        let job = db
            .get_job_by_input_path(input.to_string_lossy().as_ref())
            .await?
            .ok_or_else(|| anyhow::anyhow!("missing queued job"))?;

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
        let (jobs_tx, _) = tokio::sync::broadcast::channel(100);
        let (config_tx, _) = tokio::sync::broadcast::channel(10);
        let (system_tx, _) = tokio::sync::broadcast::channel(10);
        let event_channels = Arc::new(crate::db::EventChannels {
            jobs: jobs_tx,
            config: config_tx,
            system: system_tx,
        });
        let pipeline = Pipeline::new(
            db.clone(),
            Arc::new(Transcoder::new()),
            config,
            hardware_state,
            event_channels,
            false,
        );

        let analyzer = FfmpegAnalyzer;
        let analysis = analyzer.analyze(&input).await?;
        let plan = TranscodePlan {
            decision: TranscodeDecision::Transcode {
                reason: "resume-invalidate".to_string(),
            },
            is_remux: false,
            copy_video: false,
            output_path: Some(temp_output_path_for(&output)),
            container: "mkv".to_string(),
            requested_codec: crate::config::OutputCodec::H264,
            output_codec: Some(crate::config::OutputCodec::H264),
            encoder: Some(Encoder::H264X264),
            backend: Some(EncoderBackend::Cpu),
            rate_control: Some(RateControl::Crf { value: 21 }),
            encoder_preset: Some("ultrafast".to_string()),
            threads: 0,
            audio: AudioStreamPlan::Drop,
            audio_stream_indices: None,
            subtitles: SubtitleStreamPlan::Drop,
            filters: Vec::new(),
            allow_fallback: true,
            fallback: None,
        };

        let (session, _segments) = pipeline
            .prepare_resume_session(&job, &plan, &analysis.metadata, &output)
            .await?
            .ok_or_else(|| anyhow::anyhow!("resume session not created"))?;
        let sentinel = PathBuf::from(&session.temp_dir).join("stale.txt");
        std::fs::write(&sentinel, b"stale")?;

        std::thread::sleep(std::time::Duration::from_millis(1100));
        let bytes = std::fs::read(&input)?;
        std::fs::write(&input, &bytes)?;

        let (_new_session, segments) = pipeline
            .prepare_resume_session(&job, &plan, &analysis.metadata, &output)
            .await?
            .ok_or_else(|| anyhow::anyhow!("resume session not recreated"))?;

        assert!(!sentinel.exists());
        assert!(segments.iter().all(|segment| segment.status == "pending"));

        let _ = pipeline.purge_resume_session_state(job.id).await;
        let _ = std::fs::remove_dir_all(temp_root);
        let _ = std::fs::remove_file(db_path);
        set_test_resume_segment_length(None);
        Ok(())
    }

    #[tokio::test]
    async fn test_concat_resume_segments_behavior() -> anyhow::Result<()> {
        let temp_root = std::env::temp_dir().join(format!(
            "alchemist_pipeline_test_concat_{}",
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&temp_root)?;
        let db_path = temp_root.join("test.db");
        let db_path_str = db_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("invalid db path"))?;
        let pipeline = Pipeline::new(
            Arc::new(crate::db::Db::new(db_path_str).await?),
            Arc::new(crate::orchestrator::Transcoder::new()),
            Arc::new(tokio::sync::RwLock::new(crate::config::Config::default())),
            crate::system::hardware::HardwareState::default(),
            Arc::new(crate::db::EventChannels::default()),
            false,
        );

        let temp_dir = temp_root.join("resume_temp");
        std::fs::create_dir_all(&temp_dir)?;
        let manifest_path = temp_dir.join("segments.ffconcat");
        let temp_output = temp_root.join("output.mkv");

        let temp_dir_str = temp_dir
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("invalid temp dir path"))?
            .to_string();
        let manifest_path_str = manifest_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("invalid manifest path"))?
            .to_string();

        let session = crate::db::JobResumeSession {
            id: 1,
            job_id: 1,
            strategy: "segment".to_string(),
            plan_hash: "hash".to_string(),
            mtime_hash: "mtime".to_string(),
            temp_dir: temp_dir_str,
            concat_manifest_path: manifest_path_str,
            segment_length_secs: 10,
            status: "active".to_string(),
            created_at: chrono::Utc::now().to_string(),
            updated_at: chrono::Utc::now().to_string(),
        };

        let segment_path = temp_dir.join("00000.mkv");
        std::fs::write(&segment_path, b"dummy")?;

        let segments = vec![ResumeSegment {
            segment_index: 0,
            start_secs: 0.0,
            duration_secs: 10.0,
            temp_path: segment_path,
            status: "complete".to_string(),
            attempt_count: 1,
        }];

        // This will fail because FFmpeg isn't actually running and the segment is a dummy,
        // but we can verify the manifest content before it tries to run.
        let result = pipeline
            .concat_resume_segments(1, &session, &segments, &temp_output, "mkv")
            .await;
        assert!(result.is_err()); // Expected failure since dummy segment isn't a real MKV

        let manifest_content = std::fs::read_to_string(&manifest_path)?;
        assert!(manifest_content.contains("ffconcat version 1.0"));
        assert!(manifest_content.contains("file '"));

        // Test path escaping
        let path_with_quotes = temp_dir.join("it's a file.mkv");
        let escaped = escape_ffconcat_path(&path_with_quotes);
        assert!(escaped.contains("'\\''"));

        let _ = std::fs::remove_dir_all(temp_root);
        Ok(())
    }
}
