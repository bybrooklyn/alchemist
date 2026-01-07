use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, Semaphore};
use tracing::{info, error, warn};
use crate::db::{Db, JobState, AlchemistEvent};
use crate::Orchestrator;
use crate::config::Config;
use crate::hardware::HardwareInfo;
use crate::analyzer::Analyzer;
use crate::scanner::Scanner;

pub struct Processor {
    db: Arc<Db>,
    orchestrator: Arc<Orchestrator>,
    config: Arc<Config>,
    hw_info: Arc<Option<HardwareInfo>>,
    tx: Arc<broadcast::Sender<AlchemistEvent>>,
    semaphore: Arc<Semaphore>,
}

impl Processor {
    pub fn new(
        db: Arc<Db>,
        orchestrator: Arc<Orchestrator>,
        config: Arc<Config>,
        hw_info: Option<HardwareInfo>,
        tx: broadcast::Sender<AlchemistEvent>,
    ) -> Self {
        let concurrent_jobs = config.transcode.concurrent_jobs;
        Self {
            db,
            orchestrator,
            config,
            hw_info: Arc::new(hw_info),
            tx: Arc::new(tx),
            semaphore: Arc::new(Semaphore::new(concurrent_jobs)),
        }
    }

    pub async fn scan_and_enqueue(&self, directories: Vec<PathBuf>) -> anyhow::Result<()> {
        info!("Starting manual scan of directories: {:?}", directories);
        let scanner = Scanner::new();
        let files = scanner.scan(directories);

        for scanned_file in files {
            let mut output_path = scanned_file.path.clone();
            output_path.set_extension("av1.mkv");
            
            if let Err(e) = self.db.enqueue_job(&scanned_file.path, &output_path, scanned_file.mtime).await {
                error!("Failed to enqueue job for {:?}: {}", scanned_file.path, e);
            }
        }
        
        let _ = self.tx.send(AlchemistEvent::JobStateChanged { job_id: 0, status: JobState::Queued }); // Trigger UI refresh
        Ok(())
    }

    pub async fn run_loop(&self) {
        info!("Processor loop started.");
        loop {
            match self.db.get_next_job().await {
                Ok(Some(job)) => {
                    let permit = self.semaphore.clone().acquire_owned().await.unwrap();
                    let db = self.db.clone();
                    let orchestrator = self.orchestrator.clone();
                    let config = self.config.clone();
                    let hw_info = self.hw_info.clone();
                    let tx = self.tx.clone();

                    tokio::spawn(async move {
                        let _permit = permit;
                        let file_path = PathBuf::from(&job.input_path);
                        let output_path = PathBuf::from(&job.output_path);

                        info!("--- Processing Job {}: {:?} ---", job.id, file_path.file_name().unwrap_or_default());
                        
                        // 1. ANALYZING
                        let _ = db.update_job_status(job.id, JobState::Analyzing).await;
                        let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Analyzing });

                        match Analyzer::probe(&file_path) {
                            Ok(metadata) => {
                                let (should_encode, reason) = Analyzer::should_transcode(&file_path, &metadata, &config);
                                
                                if should_encode {
                                    info!("Decision: ENCODE Job {} - {}", job.id, reason);
                                    let _ = db.add_decision(job.id, "encode", &reason).await;
                                    let _ = tx.send(AlchemistEvent::Decision { job_id: job.id, action: "encode".to_string(), reason: reason.clone() });
                                    let _ = db.update_job_status(job.id, JobState::Encoding).await;
                                    let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Encoding });
                                    
                                    match orchestrator.transcode_to_av1(&file_path, &output_path, hw_info.as_ref().as_ref(), false, &metadata, Some((job.id, tx.clone()))).await {
                                        Ok(_) => {
                                            // Integrity & Size Reduction check
                                            let input_size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
                                            let output_size = std::fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
                                            let reduction = 1.0 - (output_size as f64 / input_size as f64);
                                            
                                            if reduction < config.transcode.size_reduction_threshold {
                                                warn!("Job {}: Size reduction gate failed ({:.2}%). Reverting.", job.id, reduction * 100.0);
                                                std::fs::remove_file(&output_path).ok();
                                                let _ = db.add_decision(job.id, "skip", "Inefficient reduction").await;
                                                let _ = db.update_job_status(job.id, JobState::Skipped).await;
                                                let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Skipped });
                                            } else {
                                                let _ = db.update_job_status(job.id, JobState::Completed).await;
                                                let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Completed });
                                            }
                                        }
                                        Err(e) => {
                                            if e.to_string() == "Cancelled" {
                                                let _ = db.update_job_status(job.id, JobState::Cancelled).await;
                                                let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Cancelled });
                                            } else {
                                                error!("Job {}: Transcode failed: {}", job.id, e);
                                                let _ = db.update_job_status(job.id, JobState::Failed).await;
                                                let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Failed });
                                            }
                                        }
                                    }
                                } else {
                                    info!("Decision: SKIP Job {} - {}", job.id, reason);
                                    let _ = db.add_decision(job.id, "skip", &reason).await;
                                    let _ = db.update_job_status(job.id, JobState::Skipped).await;
                                    let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Skipped });
                                }
                            }
                            Err(e) => {
                                error!("Job {}: Probing failed: {}", job.id, e);
                                let _ = db.update_job_status(job.id, JobState::Failed).await;
                                let _ = tx.send(AlchemistEvent::JobStateChanged { job_id: job.id, status: JobState::Failed });
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
