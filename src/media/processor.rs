use crate::Transcoder;
use crate::config::Config;
use crate::db::{Db, EventChannels, JobEvent, SystemEvent};
use crate::error::Result;
use crate::media::pipeline::Pipeline;
use crate::media::scanner::Scanner;
use crate::system::hardware::HardwareState;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::sync::{Mutex, OwnedSemaphorePermit, RwLock, Semaphore};
use tracing::{debug, error, info};

pub struct Agent {
    db: Arc<Db>,
    orchestrator: Arc<Transcoder>,
    config: Arc<RwLock<Config>>,
    hardware_state: HardwareState,
    event_channels: Arc<EventChannels>,
    semaphore: Arc<Semaphore>,
    semaphore_limit: Arc<AtomicUsize>,
    held_permits: Arc<Mutex<Vec<OwnedSemaphorePermit>>>,
    paused: Arc<AtomicBool>,
    scheduler_paused: Arc<AtomicBool>,
    draining: Arc<AtomicBool>,
    manual_override: Arc<AtomicBool>,
    pub(crate) engine_mode: Arc<tokio::sync::RwLock<crate::config::EngineMode>>,
    dry_run: bool,
    in_flight_jobs: Arc<AtomicUsize>,
    idle_notified: Arc<AtomicBool>,
    analyzing_boot: Arc<AtomicBool>,
    analysis_semaphore: Arc<tokio::sync::Semaphore>,
}

impl Agent {
    pub async fn new(
        db: Arc<Db>,
        orchestrator: Arc<Transcoder>,
        config: Arc<RwLock<Config>>,
        hardware_state: HardwareState,
        event_channels: Arc<EventChannels>,
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
            event_channels,
            semaphore: Arc::new(Semaphore::new(concurrent_jobs)),
            semaphore_limit: Arc::new(AtomicUsize::new(concurrent_jobs)),
            held_permits: Arc::new(Mutex::new(Vec::new())),
            paused: Arc::new(AtomicBool::new(false)),
            scheduler_paused: Arc::new(AtomicBool::new(false)),
            draining: Arc::new(AtomicBool::new(false)),
            manual_override: Arc::new(AtomicBool::new(false)),
            engine_mode: Arc::new(tokio::sync::RwLock::new(engine_mode)),
            dry_run,
            in_flight_jobs: Arc::new(AtomicUsize::new(0)),
            idle_notified: Arc::new(AtomicBool::new(false)),
            analyzing_boot: Arc::new(AtomicBool::new(false)),
            analysis_semaphore: Arc::new(tokio::sync::Semaphore::new(concurrent_jobs.clamp(1, 4))),
        }
    }

    pub async fn scan_and_enqueue(&self, directories: Vec<PathBuf>) -> Result<()> {
        info!("Starting manual scan of directories: {:?}", directories);

        // Notify scan started via typed channel
        let _ = self.event_channels.system.send(SystemEvent::ScanStarted);

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

        // Notify via typed channel
        let _ = self.event_channels.jobs.send(JobEvent::StateChanged {
            job_id: 0,
            status: crate::db::JobState::Queued,
        });

        let _ = self.event_channels.system.send(SystemEvent::ScanCompleted);

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
        self.idle_notified.store(false, Ordering::SeqCst);
        info!("Engine resumed.");
    }

    pub fn drain(&self) {
        // Stop accepting new jobs but finish active ones.
        // Sets draining=true. Does NOT set paused=true.
        self.draining.store(true, Ordering::SeqCst);
        self.idle_notified.store(false, Ordering::SeqCst);
        info!("Engine draining — finishing active jobs, no new jobs will start.");
    }

    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::SeqCst)
    }

    pub fn stop_drain(&self) {
        self.draining.store(false, Ordering::SeqCst);
    }

    /// Restart the engine loop without re-execing the process.
    /// Pauses the engine, cancels all in-flight jobs, resets state flags,
    /// and resumes. Cancelled jobs remain in the cancelled state.
    pub async fn restart(&self) {
        info!("Engine restart requested.");
        self.pause();

        let active_states = [
            crate::db::JobState::Encoding,
            crate::db::JobState::Remuxing,
            crate::db::JobState::Analyzing,
            crate::db::JobState::Resuming,
        ];
        for state in &active_states {
            match self.db.get_jobs_by_status(*state).await {
                Ok(jobs) => {
                    for job in jobs {
                        self.orchestrator.cancel_job(job.id);
                    }
                }
                Err(e) => {
                    error!("Restart: failed to fetch {:?} jobs: {}", state, e);
                }
            }
        }

        self.draining.store(false, Ordering::SeqCst);
        self.idle_notified.store(false, Ordering::SeqCst);
        self.resume();
        info!("Engine restart complete.");
    }

    pub fn set_boot_analyzing(&self, value: bool) {
        self.analyzing_boot.store(value, Ordering::SeqCst);
        if value {
            debug!("Boot analysis started — engine claim loop paused.");
        } else {
            debug!("Boot analysis complete — engine claim loop resumed.");
        }
    }

    pub fn is_boot_analyzing(&self) -> bool {
        self.analyzing_boot.load(Ordering::SeqCst)
    }

    /// Boot-time analysis pass. Uses blocking acquire so
    /// it always runs to completion before the engine
    /// starts processing jobs. Called once from main.rs.
    pub async fn analyze_pending_jobs_boot(&self) {
        let _permit = match self.analysis_semaphore.acquire().await {
            Ok(p) => p,
            Err(_) => {
                tracing::warn!("Auto-analysis: semaphore closed, skipping boot pass.");
                return;
            }
        };
        self._run_analysis_pass().await;
    }

    /// Watcher-triggered analysis pass. Uses try_acquire
    /// so it skips immediately if a pass is already
    /// running — the running pass will pick up newly
    /// enqueued jobs on its next loop iteration.
    /// Called from the file watcher after each enqueue.
    pub async fn analyze_pending_jobs(&self) {
        let _permit = match self.analysis_semaphore.try_acquire() {
            Ok(p) => p,
            Err(_) => {
                debug!(
                    "Auto-analysis: pass already running, \
                     skipping watcher trigger."
                );
                return;
            }
        };
        self._run_analysis_pass().await;
    }

    /// Shared analysis loop used by both boot and
    /// watcher-triggered passes. Caller holds the
    /// semaphore permit.
    async fn _run_analysis_pass(&self) {
        self.set_boot_analyzing(true);
        debug!("Auto-analysis: starting pass...");

        // NOTE: reset_interrupted_jobs is intentionally
        // NOT called here. It is a one-time startup
        // recovery operation called in main.rs before
        // this method is ever invoked. Calling it here
        // would reset jobs that are mid-analysis in a
        // concurrent pass, causing the infinite loop.

        let batch_size: i64 = 100;
        let mut total_analyzed: usize = 0;

        loop {
            let batch = match self.db.get_jobs_for_analysis_batch(0, batch_size).await {
                Ok(b) => b,
                Err(e) => {
                    error!("Auto-analysis: fetch failed: {e}");
                    break;
                }
            };

            if batch.is_empty() {
                break;
            }

            let batch_len = batch.len();
            debug!("Auto-analysis: analyzing {} job(s)...", batch_len);

            for job in batch {
                let pipeline = self.pipeline();
                match pipeline.analyze_job_only(job).await {
                    Ok(_) => {}
                    Err(e) => tracing::warn!("Auto-analysis: job failed: {e:?}"),
                }
            }

            total_analyzed += batch_len;

            // Yield between batches to avoid CPU spinning
            // and allow other tokio tasks to run.
            tokio::task::yield_now().await;
        }

        self.set_boot_analyzing(false);

        if total_analyzed == 0 {
            debug!("Auto-analysis: no jobs pending analysis.");
        } else {
            debug!(
                "Auto-analysis: complete. {} job(s) analyzed.",
                total_analyzed
            );
        }
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

        info!(
            "Updating concurrent job limit from {} to {}",
            current, new_limit
        );

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
        debug!("Agent loop started.");
        loop {
            // Block while paused OR while boot analysis runs
            if self.is_paused() || self.is_boot_analyzing() {
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }

            // Check drain BEFORE acquiring a permit to eliminate the race window
            if self.is_draining() {
                if self.in_flight_jobs.load(Ordering::SeqCst) == 0 {
                    info!(
                        "Engine drain complete — all active jobs finished. Returning to paused state."
                    );
                    self.stop_drain();
                    self.pause();
                    let _ = self
                        .event_channels
                        .system
                        .send(crate::db::SystemEvent::EngineStatusChanged);
                }
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
            debug!(
                "Worker slot acquired (in_flight={}, limit={})",
                self.in_flight_jobs.load(Ordering::SeqCst),
                self.concurrent_jobs_limit()
            );

            // Re-check drain after permit acquisition (belt-and-suspenders)
            if self.is_draining() {
                drop(permit);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }

            match self.db.claim_next_job().await {
                Ok(Some(job)) => {
                    self.idle_notified.store(false, Ordering::SeqCst);
                    let next_in_flight = self.in_flight_jobs.fetch_add(1, Ordering::SeqCst) + 1;
                    info!(
                        "Claimed job {} for processing (in_flight={}, limit={})",
                        job.id,
                        next_in_flight,
                        self.concurrent_jobs_limit()
                    );
                    let agent = self.clone();
                    let counter = self.in_flight_jobs.clone();
                    tokio::spawn(async move {
                        struct InFlightGuard(Arc<AtomicUsize>);
                        impl Drop for InFlightGuard {
                            fn drop(&mut self) {
                                self.0.fetch_sub(1, Ordering::SeqCst);
                            }
                        }

                        let _guard = InFlightGuard(counter);
                        let _permit = permit;
                        if let Err(e) = agent.process_job(job).await {
                            error!("Job processing error: {}", e);
                        }
                        // _guard drops here automatically, even on panic
                    });
                }
                Ok(None) => {
                    debug!(
                        "No queued job available (in_flight={}, limit={})",
                        self.in_flight_jobs.load(Ordering::SeqCst),
                        self.concurrent_jobs_limit()
                    );
                    if self.in_flight_jobs.load(Ordering::SeqCst) == 0
                        && !self.idle_notified.swap(true, Ordering::SeqCst)
                    {
                        let _ = self.event_channels.system.send(SystemEvent::EngineIdle);
                    }
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
            self.event_channels.clone(),
            self.dry_run,
        )
    }

    /// Gracefully shutdown the agent.
    /// Cancels active jobs immediately and returns quickly.
    pub async fn graceful_shutdown(&self) {
        info!("Initiating rapid shutdown...");

        // Stop accepting new jobs
        self.pause();

        // Immediately force cancel remaining jobs
        let cancelled = self.orchestrator.cancel_all_jobs();
        if cancelled > 0 {
            tracing::warn!(
                "Fast shutdown requested. Forcefully cancelled {} job(s).",
                cancelled
            );
            // Give FFmpeg processes a moment to terminate and Tokio to flush DB statuses
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        info!("Rapid shutdown complete.");
    }
}
