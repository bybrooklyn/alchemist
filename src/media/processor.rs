use crate::config::Config;
use crate::db::{AlchemistEvent, Db, Job, JobState};
use crate::error::Result;
use crate::media::analyzer::FfmpegAnalyzer;
use crate::media::executor::FfmpegExecutor;
use crate::media::pipeline::{
    Analyzer as AnalyzerTrait, Executor as ExecutorTrait, Planner as PlannerTrait,
};
use crate::media::planner::BasicPlanner;
use crate::media::scanner::Scanner;
use crate::system::hardware::HardwareInfo;
use crate::Transcoder;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Semaphore};
use tracing::{error, info, warn};

pub struct Agent {
    db: Arc<Db>,
    orchestrator: Arc<Transcoder>,
    config: Arc<RwLock<Config>>,
    hw_info: Arc<Option<HardwareInfo>>,
    tx: Arc<broadcast::Sender<AlchemistEvent>>,
    semaphore: Arc<Semaphore>,
    paused: Arc<AtomicBool>,
    scheduler_paused: Arc<AtomicBool>,
    dry_run: bool,
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
            let mut output_path = scanned_file.path.clone();
            let stem = output_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy();
            let new_filename = format!(
                "{}{}.{}",
                stem, settings.output_suffix, settings.output_extension
            );
            output_path.set_file_name(new_filename);

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

    pub async fn run_loop(self: Arc<Self>) {
        info!("Agent loop started.");
        loop {
            if self.is_paused() {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }

            match self.db.get_next_job().await {
                Ok(Some(job)) => {
                    let permit = self.semaphore.clone().acquire_owned().await.unwrap();
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
        let planner = BasicPlanner::new(
            Arc::new(config_snapshot.clone()),
            self.hw_info.as_ref().clone(),
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
                self.finalize_job(job, &file_path, &output_path, start_time)
                    .await
            }
            Err(e) => {
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
        let _ = self.db.update_job_status(job_id, status).await;
        let _ = self
            .tx
            .send(AlchemistEvent::JobStateChanged { job_id, status });
        Ok(())
    }

    async fn finalize_job(
        &self,
        job: Job,
        input_path: &std::path::Path,
        output_path: &std::path::Path,
        start_time: std::time::Instant,
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

        // 2. QUALITY GATE (VMAF)
        let mut vmaf_score = None;
        if config.quality.enable_vmaf {
            info!("[Job {}] Phase 2: Computing VMAF quality score...", job_id);
            match crate::media::ffmpeg::QualityScore::compute(input_path, output_path) {
                Ok(score) => {
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
                Err(e) => {
                    warn!("[Job {}] VMAF computation failed: {}", job_id, e);
                }
            }
        }

        // Finalize results
        let bitrate = (output_size as f64 * 8.0) / (encode_duration * 1000.0); // Rough estimate
        let _ = self
            .db
            .save_encode_stats(
                job_id,
                input_size,
                output_size,
                reduction,
                encode_duration,
                1.0, // Speed calculation could be refined
                bitrate,
                vmaf_score,
            )
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
