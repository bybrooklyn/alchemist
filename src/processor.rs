use crate::analyzer::Analyzer;
use crate::config::Config;
use crate::db::{AlchemistEvent, Db, JobState};
use crate::error::Result;
use crate::hardware::HardwareInfo;
use crate::scanner::Scanner;
use crate::Transcoder;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Semaphore};
use tracing::{error, info, warn};

pub struct Agent {
    db: Arc<Db>,
    orchestrator: Arc<Transcoder>,
    config: Arc<Config>,
    hw_info: Arc<Option<HardwareInfo>>,
    tx: Arc<broadcast::Sender<AlchemistEvent>>,
    semaphore: Arc<Semaphore>,
    dry_run: bool,
}

impl Agent {
    pub fn new(
        db: Arc<Db>,
        orchestrator: Arc<Transcoder>,
        config: Arc<Config>,
        hw_info: Option<HardwareInfo>,
        tx: broadcast::Sender<AlchemistEvent>,
        dry_run: bool,
    ) -> Self {
        let concurrent_jobs = config.transcode.concurrent_jobs;
        Self {
            db,
            orchestrator,
            config,
            hw_info: Arc::new(hw_info),
            tx: Arc::new(tx),
            semaphore: Arc::new(Semaphore::new(concurrent_jobs)),
            dry_run,
        }
    }

    pub async fn scan_and_enqueue(&self, directories: Vec<PathBuf>) -> Result<()> {
        info!("Starting manual scan of directories: {:?}", directories);
        let scanner = Scanner::new();
        let files = scanner.scan(directories);

        for scanned_file in files {
            let mut output_path = scanned_file.path.clone();
            output_path.set_extension("av1.mkv");

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

    pub async fn run_loop(&self) {
        info!("Agent loop started.");
        loop {
            match self.db.get_next_job().await {
                Ok(Some(job)) => {
                    let permit = self.semaphore.clone().acquire_owned().await.unwrap();
                    let db = self.db.clone();
                    let orchestrator = self.orchestrator.clone();
                    let config = self.config.clone();
                    let hw_info = self.hw_info.clone();
                    let tx = self.tx.clone();
                    let dry_run = self.dry_run;

                    tokio::spawn(async move {
                        let _permit = permit;
                        let file_path = PathBuf::from(&job.input_path);
                        let output_path = PathBuf::from(&job.output_path);

                        let file_name = file_path.file_name().unwrap_or_default();
                        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                        info!("📹 Processing Job #{}: {:?}", job.id, file_name);
                        info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

                        let start_time = std::time::Instant::now();

                        // 1. ANALYZING
                        info!("[Job {}] Phase 1: Analyzing media...", job.id);
                        let _ = db.update_job_status(job.id, JobState::Analyzing).await;
                        let _ = tx.send(AlchemistEvent::JobStateChanged {
                            job_id: job.id,
                            status: JobState::Analyzing,
                        });

                        let analyze_start = std::time::Instant::now();
                        match Analyzer::probe(&file_path) {
                            Ok(metadata) => {
                                let analyze_duration = analyze_start.elapsed();
                                info!(
                                    "[Job {}] Analysis complete in {:.2}s",
                                    job.id,
                                    analyze_duration.as_secs_f64()
                                );

                                // Get video stream info
                                if let Some(video_stream) =
                                    metadata.streams.iter().find(|s| s.codec_type == "video")
                                {
                                    if let (Some(width), Some(height)) =
                                        (video_stream.width, video_stream.height)
                                    {
                                        info!("[Job {}] Resolution: {}x{}", job.id, width, height);
                                    }
                                    info!("[Job {}] Codec: {}", job.id, video_stream.codec_name);
                                }

                                let (should_encode, reason) =
                                    Analyzer::should_transcode(&file_path, &metadata, &config);

                                if should_encode {
                                    info!("Decision: ENCODE Job {} - {}", job.id, reason);
                                    let _ = db.add_decision(job.id, "encode", &reason).await;
                                    let _ = tx.send(AlchemistEvent::Decision {
                                        job_id: job.id,
                                        action: "encode".to_string(),
                                        reason: reason.clone(),
                                    });
                                    let _ = db.update_job_status(job.id, JobState::Encoding).await;
                                    let _ = tx.send(AlchemistEvent::JobStateChanged {
                                        job_id: job.id,
                                        status: JobState::Encoding,
                                    });

                                    match orchestrator
                                        .transcode_to_av1(
                                            &file_path,
                                            &output_path,
                                            hw_info.as_ref().as_ref(),
                                            &config.hardware.cpu_preset,
                                            dry_run,
                                            &metadata,
                                            Some((job.id, tx.clone())),
                                        )
                                        .await
                                    {
                                        Ok(_) => {
                                            // Integrity & Size Reduction check
                                            let input_size = std::fs::metadata(&file_path)
                                                .map(|m| m.len())
                                                .unwrap_or(0);
                                            let output_size = std::fs::metadata(&output_path)
                                                .map(|m| m.len())
                                                .unwrap_or(0);
                                            let reduction =
                                                1.0 - (output_size as f64 / input_size as f64);

                                            if reduction < config.transcode.size_reduction_threshold
                                            {
                                                warn!(
                                                    "Job {}: Size reduction gate failed ({:.2}%). Reverting.",
                                                    job.id,
                                                    reduction * 100.0
                                                );
                                                std::fs::remove_file(&output_path).ok();
                                                let _ = db
                                                    .add_decision(
                                                        job.id,
                                                        "skip",
                                                        "Inefficient reduction",
                                                    )
                                                    .await;
                                                let _ = db
                                                    .update_job_status(job.id, JobState::Skipped)
                                                    .await;
                                                let _ = tx.send(AlchemistEvent::JobStateChanged {
                                                    job_id: job.id,
                                                    status: JobState::Skipped,
                                                });
                                            } else {
                                                let encode_duration = start_time.elapsed();
                                                info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                                                info!("✅ Job #{} COMPLETED", job.id);
                                                info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                                                info!(
                                                    "  Input Size:  {} MB",
                                                    input_size / 1_048_576
                                                );
                                                info!(
                                                    "  Output Size: {} MB",
                                                    output_size / 1_048_576
                                                );
                                                info!("  Reduction:   {:.1}%", reduction * 100.0);
                                                info!(
                                                    "  Duration:    {:.2}s",
                                                    encode_duration.as_secs_f64()
                                                );
                                                info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                                                let _ = db
                                                    .update_job_status(job.id, JobState::Completed)
                                                    .await;
                                                let _ = tx.send(AlchemistEvent::JobStateChanged {
                                                    job_id: job.id,
                                                    status: JobState::Completed,
                                                });
                                            }
                                        }
                                        Err(e) => {
                                            if e.to_string() == "Cancelled" {
                                                let _ = db
                                                    .update_job_status(job.id, JobState::Cancelled)
                                                    .await;
                                                let _ = tx.send(AlchemistEvent::JobStateChanged {
                                                    job_id: job.id,
                                                    status: JobState::Cancelled,
                                                });
                                            } else {
                                                error!("Job {}: Transcode failed: {}", job.id, e);
                                                let _ = db
                                                    .update_job_status(job.id, JobState::Failed)
                                                    .await;
                                                let _ = tx.send(AlchemistEvent::JobStateChanged {
                                                    job_id: job.id,
                                                    status: JobState::Failed,
                                                });
                                            }
                                        }
                                    }
                                } else {
                                    info!("Decision: SKIP Job {} - {}", job.id, reason);
                                    let _ = db.add_decision(job.id, "skip", &reason).await;
                                    let _ = db.update_job_status(job.id, JobState::Skipped).await;
                                    let _ = tx.send(AlchemistEvent::JobStateChanged {
                                        job_id: job.id,
                                        status: JobState::Skipped,
                                    });
                                }
                            }
                            Err(e) => {
                                error!("Job {}: Probing failed: {}", job.id, e);
                                let _ = db.update_job_status(job.id, JobState::Failed).await;
                                let _ = tx.send(AlchemistEvent::JobStateChanged {
                                    job_id: job.id,
                                    status: JobState::Failed,
                                });
                            }
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
}
