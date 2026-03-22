use crate::config::Config;
use crate::db::{AlchemistEvent, Db};
use crate::error::Result;
use crate::media::pipeline::Pipeline;
use crate::media::scanner::Scanner;
use crate::system::hardware::HardwareState;
use crate::Transcoder;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, OwnedSemaphorePermit, RwLock, Semaphore};
use tracing::{error, info};

pub struct Agent {
    db: Arc<Db>,
    orchestrator: Arc<Transcoder>,
    config: Arc<RwLock<Config>>,
    hardware_state: HardwareState,
    tx: Arc<broadcast::Sender<AlchemistEvent>>,
    semaphore: Arc<Semaphore>,
    semaphore_limit: Arc<AtomicUsize>,
    held_permits: Arc<Mutex<Vec<OwnedSemaphorePermit>>>,
    paused: Arc<AtomicBool>,
    scheduler_paused: Arc<AtomicBool>,
    draining: Arc<AtomicBool>,
    manual_override: Arc<AtomicBool>,
    pub(crate) engine_mode: Arc<tokio::sync::RwLock<crate::config::EngineMode>>,
    dry_run: bool,
}

impl Agent {
    pub async fn new(
        db: Arc<Db>,
        orchestrator: Arc<Transcoder>,
        config: Arc<RwLock<Config>>,
        hardware_state: HardwareState,
        tx: broadcast::Sender<AlchemistEvent>,
        dry_run: bool,
    ) -> Self {
        // Read config asynchronously to avoid blocking atomic in async runtime
        let config_read = config.read().await;
        let concurrent_jobs = config_read.transcode.concurrent_jobs;
        let engine_mode = config_read.system.engine_mode;
        drop(config_read);

        Self {
            db,
            orchestrator,
            config,
            hardware_state,
            tx: Arc::new(tx),
            semaphore: Arc::new(Semaphore::new(concurrent_jobs)),
            semaphore_limit: Arc::new(AtomicUsize::new(concurrent_jobs)),
            held_permits: Arc::new(Mutex::new(Vec::new())),
            paused: Arc::new(AtomicBool::new(false)),
            scheduler_paused: Arc::new(AtomicBool::new(false)),
            draining: Arc::new(AtomicBool::new(false)),
            manual_override: Arc::new(AtomicBool::new(false)),
            engine_mode: Arc::new(tokio::sync::RwLock::new(engine_mode)),
            dry_run,
        }
    }

    pub async fn scan_and_enqueue(&self, directories: Vec<PathBuf>) -> Result<()> {
        info!("Starting manual scan of directories: {:?}", directories);
        let files = tokio::task::spawn_blocking(move || {
            let scanner = Scanner::new();
            scanner.scan(directories)
        })
        .await
        .map_err(|e| crate::error::AlchemistError::Unknown(format!("scan task failed: {}", e)))?;

        let pipeline = self.pipeline();

        for scanned_file in files {
            let path = scanned_file.path.clone();
            if let Err(e) = pipeline.enqueue_discovered(scanned_file).await {
                error!("Failed to enqueue job for {:?}: {}", path, e);
            }
        }

        let _ = self.tx.send(AlchemistEvent::JobStateChanged {
            job_id: 0,
            status: crate::db::JobState::Queued,
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

    pub fn concurrent_jobs_limit(&self) -> usize {
        self.semaphore_limit.load(Ordering::SeqCst)
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

    pub fn drain(&self) {
        // Stop accepting new jobs but finish active ones.
        // Sets draining=true. Does NOT set paused=true.
        self.draining.store(true, Ordering::SeqCst);
        info!("Engine draining — finishing active jobs, no new jobs will start.");
    }

    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::SeqCst)
    }

    pub fn stop_drain(&self) {
        self.draining.store(false, Ordering::SeqCst);
    }

    pub async fn current_mode(&self) -> crate::config::EngineMode {
        *self.engine_mode.read().await
    }

    /// Apply a resource mode. Computes the correct concurrent
    /// job count from cpu_count and calls set_concurrent_jobs.
    /// Clears manual override flag.
    pub async fn apply_mode(&self, mode: crate::config::EngineMode, cpu_count: usize) {
        let jobs = mode.concurrent_jobs_for_cpu_count(cpu_count);
        *self.engine_mode.write().await = mode;
        self.set_manual_override(false);
        self.set_concurrent_jobs(jobs).await;
        info!(
            "Engine mode set to '{}' → {} concurrent jobs ({} CPUs)",
            mode.as_str(),
            jobs,
            cpu_count
        );
    }

    pub fn set_manual_override(&self, value: bool) {
        self.manual_override.store(value, Ordering::SeqCst);
    }

    pub fn is_manual_override(&self) -> bool {
        self.manual_override.load(Ordering::SeqCst)
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

            let permit = match self.semaphore.clone().acquire_owned().await {
                Ok(permit) => permit,
                Err(e) => {
                    error!("Failed to acquire job permit: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
            };

            if self.is_draining() {
                drop(permit);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }

            match self.db.claim_next_job().await {
                Ok(Some(job)) => {
                    let agent = self.clone();

                    tokio::spawn(async move {
                        let _permit = permit;
                        if let Err(e) = agent.process_job(job).await {
                            error!("Job processing error: {}", e);
                        }
                    });
                }
                Ok(None) => {
                    drop(permit);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
                Err(e) => {
                    drop(permit);
                    error!("Database error in processor loop: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    pub async fn process_job(&self, job: crate::db::Job) -> Result<()> {
        let pipeline = self.pipeline();
        pipeline
            .process_job(job)
            .await
            .map_err(|failure| crate::error::AlchemistError::Unknown(format!("{:?}", failure)))
    }

    fn pipeline(&self) -> Pipeline {
        Pipeline::new(
            self.db.clone(),
            self.orchestrator.clone(),
            self.config.clone(),
            self.hardware_state.clone(),
            self.tx.clone(),
            self.dry_run,
        )
    }
}
