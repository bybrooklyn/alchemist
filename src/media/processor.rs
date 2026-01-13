use crate::config::{Config, OutputCodec};
use crate::db::{AlchemistEvent, Db, EncodeStatsInput, Job, JobState};
use crate::error::Result;
use crate::media::analyzer::FfmpegAnalyzer;
use crate::media::executor::FfmpegExecutor;
use crate::media::pipeline::{
    Analyzer as AnalyzerTrait, Executor as ExecutorTrait, Planner as PlannerTrait,
};
use crate::media::planner::BasicPlanner;
use crate::media::scanner::Scanner;
use crate::system::hardware::HardwareInfo;
use crate::telemetry::{encoder_label, hardware_label, resolution_bucket, TelemetryEvent};
use crate::Transcoder;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, OwnedSemaphorePermit, RwLock, Semaphore};
use tracing::{error, info, warn};

pub struct Agent {
    db: Arc<Db>,
    orchestrator: Arc<Transcoder>,
    config: Arc<RwLock<Config>>,
    hw_info: Arc<Option<HardwareInfo>>,
    tx: Arc<broadcast::Sender<AlchemistEvent>>,
    semaphore: Arc<Semaphore>,
    semaphore_limit: Arc<AtomicUsize>,
    held_permits: Arc<Mutex<Vec<OwnedSemaphorePermit>>>,
    paused: Arc<AtomicBool>,
    scheduler_paused: Arc<AtomicBool>,
    dry_run: bool,
}

struct TelemetryEventParams<'a> {
    telemetry_enabled: bool,
    output_codec: OutputCodec,
    metadata: &'a crate::media::pipeline::MediaMetadata,
    event_type: &'static str,
    status: Option<&'static str>,
    failure_reason: Option<&'static str>,
    input_size_bytes: Option<u64>,
    output_size_bytes: Option<u64>,
    duration_ms: Option<u64>,
    speed_factor: Option<f64>,
}

impl Agent {
    pub async fn new(
        db: Arc<Db>,
        orchestrator: Arc<Transcoder>,
        config: Arc<RwLock<Config>>,
        hw_info: Option<HardwareInfo>,
        tx: broadcast::Sender<AlchemistEvent>,
        dry_run: bool,
    ) -> Self {
        // Read config asynchronously to avoid blocking atomic in async runtime
        let config_read = config.read().await;
        let concurrent_jobs = config_read.transcode.concurrent_jobs;
        drop(config_read);

        Self {
            db,
            orchestrator,
            config,
            hw_info: Arc::new(hw_info),
            tx: Arc::new(tx),
            semaphore: Arc::new(Semaphore::new(concurrent_jobs)),
            semaphore_limit: Arc::new(AtomicUsize::new(concurrent_jobs)),
            held_permits: Arc::new(Mutex::new(Vec::new())),
            paused: Arc::new(AtomicBool::new(false)),
            scheduler_paused: Arc::new(AtomicBool::new(false)),
            dry_run,
        }
    }

    pub async fn scan_and_enqueue(&self, directories: Vec<PathBuf>) -> Result<()> {
        info!("Starting manual scan of directories: {:?}", directories);
        let scanner = Scanner::new();
        let files = scanner.scan(directories);

        // Get output settings
        let settings = match self.db.get_file_settings().await {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to fetch file settings, using defaults: {}", e);
                crate::db::FileSettings {
                    id: 1,
                    delete_source: false,
                    output_extension: "mkv".to_string(),
                    output_suffix: "-alchemist".to_string(),
                    replace_strategy: "keep".to_string(),
                }
            }
        };

        for scanned_file in files {
            let output_path = settings.output_path_for(&scanned_file.path);

            if output_path.exists() && !settings.should_replace_existing_output() {
                info!(
                    "Skipping {:?} (output exists, replace_strategy = keep)",
                    scanned_file.path
                );
                continue;
            }

            if let Err(e) = self
                .db
                .enqueue_job(&scanned_file.path, &output_path, scanned_file.mtime)
                .await
            {
                error!("Failed to enqueue job for {:?}: {}", scanned_file.path, e);
            }
        }

        let _ = self.tx.send(AlchemistEvent::JobStateChanged {
            job_id: 0,
            status: JobState::Queued,
        }); // Trigger UI refresh
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

        tokio::spawn(async move {
            crate::telemetry::send_event(event).await;
        });
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst) || self.scheduler_paused.load(Ordering::SeqCst)
    }

    pub fn is_manual_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn is_scheduler_paused(&self) -> bool {
        self.scheduler_paused.load(Ordering::SeqCst)
    }

    pub fn set_scheduler_paused(&self, paused: bool) {
        let current = self.scheduler_paused.load(Ordering::SeqCst);
        if current != paused {
            self.scheduler_paused.store(paused, Ordering::SeqCst);
            if paused {
                info!("Engine paused by scheduler.");
            } else {
                info!("Engine resumed by scheduler.");
            }
        }
    }

    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
        info!("Engine paused.");
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
        info!("Engine resumed.");
    }

    pub async fn set_concurrent_jobs(&self, new_limit: usize) {
        if new_limit == 0 {
            return;
        }

        let current = self.semaphore_limit.load(Ordering::SeqCst);
        if new_limit == current {
            return;
        }

        if new_limit > current {
            let mut held = self.held_permits.lock().await;
            let mut increase = new_limit - current;

            if !held.is_empty() {
                let release = increase.min(held.len());
                held.drain(0..release);
                increase -= release;
            }

            if increase > 0 {
                self.semaphore.add_permits(increase);
            }

            self.semaphore_limit.store(new_limit, Ordering::SeqCst);
            return;
        }

        let reduce_by = current - new_limit;
        self.semaphore_limit.store(new_limit, Ordering::SeqCst);

        let semaphore = self.semaphore.clone();
        let held = self.held_permits.clone();
        let limit = self.semaphore_limit.clone();
        let target_limit = new_limit;
        tokio::spawn(async move {
            let mut acquired = Vec::with_capacity(reduce_by);
            for _ in 0..reduce_by {
                match semaphore.clone().acquire_owned().await {
                    Ok(permit) => {
                        if limit.load(Ordering::SeqCst) > target_limit {
                            drop(permit);
                            break;
                        }
                        acquired.push(permit);
                    }
                    Err(_) => break,
                }
            }
            if acquired.is_empty() || limit.load(Ordering::SeqCst) > target_limit {
                return;
            }
            let mut held_guard = held.lock().await;
            held_guard.extend(acquired);
        });
    }

    pub async fn run_loop(self: Arc<Self>) {
        info!("Agent loop started.");
        loop {
            if self.is_paused() {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }

            match self.db.claim_next_job().await {
                Ok(Some(job)) => {
                    let permit = match self.semaphore.clone().acquire_owned().await {
                        Ok(permit) => permit,
                        Err(e) => {
                            error!("Failed to acquire job permit: {}", e);
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            continue;
                        }
                    };
                    let agent = self.clone();

                    tokio::spawn(async move {
                        let _permit = permit;
                        if let Err(e) = agent.process_job(job).await {
                            error!("Job processing error: {}", e);
                        }
                    });
                }
                Ok(None) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
                Err(e) => {
                    error!("Database error in processor loop: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    pub async fn process_job(&self, job: crate::db::Job) -> Result<()> {
        let file_path = PathBuf::from(&job.input_path);
        let output_path = PathBuf::from(&job.output_path);

        if file_path == output_path {
            error!(
                "Job {}: Output path matches input path; refusing to overwrite source.",
                job.id
            );
            let _ = self
                .db
                .add_decision(job.id, "skip", "Output path matches input path")
                .await;
            self.update_job_state(job.id, JobState::Skipped).await?;
            return Ok(());
        }

        if let Ok(settings) = self.db.get_file_settings().await {
            if output_path.exists() && !settings.should_replace_existing_output() {
                info!(
                    "Job {}: Output exists and replace_strategy is keep. Skipping.",
                    job.id
                );
                let _ = self
                    .db
                    .add_decision(job.id, "skip", "Output already exists")
                    .await;
                self.update_job_state(job.id, JobState::Skipped).await?;
                return Ok(());
            }
        }

        let file_name = file_path.file_name().unwrap_or_default();
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("📹 Processing Job #{}: {:?}", job.id, file_name);
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let start_time = std::time::Instant::now();

        // 1. ANALYZING
        info!("[Job {}] Phase 1: Analyzing media...", job.id);
        self.db.increment_attempt_count(job.id).await?;
        self.update_job_state(job.id, JobState::Analyzing).await?;

        let analyzer = FfmpegAnalyzer;
        let analyze_start = std::time::Instant::now();
        let metadata = match analyzer.analyze(&file_path).await {
            Ok(m) => m,
            Err(e) => {
                error!("Job {}: Probing failed: {}", job.id, e);
                self.update_job_state(job.id, JobState::Failed).await?;
                return Err(e);
            }
        };

        let analyze_duration = analyze_start.elapsed();
        info!(
            "[Job {}] Analysis complete in {:.2}s",
            job.id,
            analyze_duration.as_secs_f64()
        );

        // Get video stream info
        info!(
            "[Job {}] Resolution: {}x{}",
            job.id, metadata.width, metadata.height
        );
        info!("[Job {}] Codec: {}", job.id, metadata.codec_name);

        let config_snapshot = self.config.read().await.clone();
        let encoder_caps = Arc::new(crate::media::ffmpeg::encoder_caps_clone());
        let planner = BasicPlanner::new(
            Arc::new(config_snapshot.clone()),
            self.hw_info.as_ref().clone(),
            encoder_caps,
        );
        let decision = planner.plan(&metadata).await?;
        let should_encode = decision.action == "encode";
        let reason = decision.reason.clone();

        if !should_encode {
            info!("Decision: SKIP Job {} - {}", job.id, &reason);
            let _ = self.db.add_decision(job.id, "skip", &reason).await;
            self.update_job_state(job.id, JobState::Skipped).await?;
            return Ok(());
        }

        info!("Decision: ENCODE Job {} - {}", job.id, &reason);
        let _ = self.db.add_decision(job.id, "encode", &reason).await;
        let _ = self.tx.send(AlchemistEvent::Decision {
            job_id: job.id,
            action: "encode".to_string(),
            reason: reason.clone(),
        });

        self.update_job_state(job.id, JobState::Encoding).await?;

        self.emit_telemetry_event(TelemetryEventParams {
            telemetry_enabled: config_snapshot.system.enable_telemetry,
            output_codec: config_snapshot.transcode.output_codec,
            metadata: &metadata,
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
            Arc::new(config_snapshot.clone()), // Use snapshot
            self.hw_info.as_ref().clone(),     // Option<HardwareInfo>
            self.tx.clone(),
            self.dry_run,
        );

        match executor
            .execute(
                &job,
                &crate::db::Decision {
                    id: 0, // Mock decision for now, relying on job
                    job_id: job.id,
                    action: "encode".to_string(),
                    reason: reason.clone(),
                    created_at: chrono::Utc::now(),
                },
                &metadata,
            )
            .await
        {
            Ok(_) => {
                self.finalize_job(job, &file_path, &output_path, start_time, &metadata)
                    .await
            }
            Err(e) => {
                if output_path.exists() {
                    if let Err(err) = tokio::fs::remove_file(&output_path).await {
                        warn!(
                            "Job {}: Failed to remove partial output {:?}: {}",
                            job.id, output_path, err
                        );
                    } else {
                        info!("Job {}: Removed partial output {:?}", job.id, output_path);
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
                    metadata: &metadata,
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
                    self.update_job_state(job.id, JobState::Cancelled).await
                } else {
                    error!("Job {}: Transcode failed: {}", job.id, e);
                    self.update_job_state(job.id, JobState::Failed).await?;
                    Err(e)
                }
            }
        }
    }

    async fn update_job_state(&self, job_id: i64, status: JobState) -> Result<()> {
        if let Err(e) = self.db.update_job_status(job_id, status).await {
            error!("Failed to update job {} status {:?}: {}", job_id, status, e);
            return Err(e);
        }
        let _ = self.tx.send(AlchemistEvent::JobStateChanged { job_id, status });
        Ok(())
    }

    async fn finalize_job(
        &self,
        job: Job,
        input_path: &std::path::Path,
        output_path: &std::path::Path,
        start_time: std::time::Instant,
        metadata: &crate::media::pipeline::MediaMetadata,
    ) -> Result<()> {
        let job_id = job.id;
        // Integrity & Size Reduction check
        let input_metadata = match std::fs::metadata(input_path) {
            Ok(m) => m,
            Err(e) => {
                error!("Job {}: Failed to get input metadata: {}", job_id, e);
                self.update_job_state(job_id, JobState::Failed).await?;
                return Err(e.into());
            }
        };
        let input_size = input_metadata.len();

        let output_metadata = match std::fs::metadata(output_path) {
            Ok(m) => m,
            Err(e) => {
                error!("Job {}: Failed to get output metadata: {}", job_id, e);
                self.update_job_state(job_id, JobState::Failed).await?;
                return Err(e.into());
            }
        };
        let output_size = output_metadata.len();

        if input_size == 0 {
            error!("Job {}: Input file is empty. Finalizing as failed.", job_id);
            self.update_job_state(job_id, JobState::Failed).await?;
            return Ok(());
        }

        let reduction = 1.0 - (output_size as f64 / input_size as f64);
        let encode_duration = start_time.elapsed().as_secs_f64();

        let config = self.config.read().await;
        let telemetry_enabled = config.system.enable_telemetry;
        let output_codec = config.transcode.output_codec;

        // Check reduction threshold
        if output_size == 0 || reduction < config.transcode.size_reduction_threshold {
            warn!(
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
            self.update_job_state(job_id, JobState::Skipped).await?;
            return Ok(());
        }

        // 2. QUALITY GATE (VMAF) - run in spawn_blocking to avoid blocking async runtime
        let mut vmaf_score = None;
        if config.quality.enable_vmaf {
            info!("[Job {}] Phase 2: Computing VMAF quality score...", job_id);
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
                        info!("[Job {}] VMAF Score: {:.2}", job_id, s);
                        if s < config.quality.min_vmaf_score && config.quality.revert_on_low_quality
                        {
                            warn!(
                                "Job {}: Quality gate failed ({:.2} < {}). Reverting.",
                                job_id, s, config.quality.min_vmaf_score
                            );
                            let _ = std::fs::remove_file(output_path);
                            let _ = self
                                .db
                                .add_decision(job_id, "skip", "Low quality (VMAF)")
                                .await;
                            self.update_job_state(job_id, JobState::Skipped).await?;
                            return Ok(());
                        }
                    }
                }
                Ok(Err(e)) => {
                    warn!("[Job {}] VMAF computation failed: {}", job_id, e);
                }
                Err(e) => {
                    warn!("[Job {}] VMAF spawn_blocking failed: {}", job_id, e);
                }
            }
        }

        // Get actual media duration for accurate metrics (using spawn_blocking)
        let media_duration = {
            let input_clone = input_path.to_path_buf();
            tokio::task::spawn_blocking(move || {
                crate::media::analyzer::Analyzer::probe(&input_clone)
                    .ok()
                    .and_then(|meta| meta.format.duration.parse::<f64>().ok())
                    .unwrap_or(0.0)
            })
            .await
            .unwrap_or(0.0)
        };

        // Calculate accurate encode_speed: how much faster than realtime (e.g., 2.0x)
        let encode_speed = if encode_duration > 0.0 && media_duration > 0.0 {
            media_duration / encode_duration
        } else {
            0.0
        };

        // Calculate avg_bitrate from output size and actual media duration (not encode time)
        let avg_bitrate_kbps = if media_duration > 0.0 {
            (output_size as f64 * 8.0) / (media_duration * 1000.0)
        } else {
            0.0
        };

        let _ = self
            .db
            .save_encode_stats(EncodeStatsInput {
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

        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("✅ Job #{} COMPLETED", job_id);
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        info!("  Input Size:  {} MB", input_size / 1_048_576);
        info!("  Output Size: {} MB", output_size / 1_048_576);
        info!("  Reduction:   {:.1}%", reduction * 100.0);
        if let Some(s) = vmaf_score {
            info!("  VMAF Score:  {:.2}", s);
        }
        info!("  Duration:    {:.2}s", encode_duration);
        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        self.update_job_state(job_id, JobState::Completed).await?;

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

        // Handle File Deletion Policy
        if let Ok(settings) = self.db.get_file_settings().await {
            if settings.delete_source {
                info!(
                    "Job {}: 'Delete Source' is enabled. Removing input file: {:?}",
                    job_id, input_path
                );
                if let Err(e) = tokio::fs::remove_file(input_path).await {
                    error!("Job {}: Failed to delete input file: {}", job_id, e);
                } else {
                    info!("Job {}: Input file deleted successfully.", job_id);
                }
            }
        }

        Ok(())
    }
}
