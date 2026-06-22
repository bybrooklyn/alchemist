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
use tracing::{debug, error, info, warn};

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
    /// AUTO-3 disk guardrail: set when the engine is holding jobs because the
    /// next job's output filesystem is below the configured free-space minimum.
    disk_blocked: Arc<AtomicBool>,
    disk_block_reason: Arc<std::sync::Mutex<Option<String>>>,
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
            disk_blocked: Arc::new(AtomicBool::new(false)),
            disk_block_reason: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Whether the engine is currently holding jobs because of the AUTO-3 disk
    /// guardrail (the next queued job's output filesystem is below the
    /// configured free-space minimum).
    pub fn is_disk_blocked(&self) -> bool {
        self.disk_blocked.load(Ordering::SeqCst)
    }

    /// The human-readable reason the disk guardrail is engaged, if it is.
    pub fn disk_block_reason(&self) -> Option<String> {
        self.disk_block_reason
            .lock()
            .ok()
            .and_then(|reason| reason.clone())
    }

    fn engage_disk_block(&self, reason: String) {
        if let Ok(mut current) = self.disk_block_reason.lock() {
            *current = Some(reason.clone());
        }
        // Only log/notify on the transition into the blocked state so a low
        // disk doesn't spam the log (or notifications) on every loop iteration.
        if !self.disk_blocked.swap(true, Ordering::SeqCst) {
            warn!("Engine holding jobs — disk guardrail: {reason}");
            let _ = self
                .event_channels
                .system
                .send(SystemEvent::EngineStatusChanged);
            let _ = self
                .event_channels
                .system
                .send(SystemEvent::DiskSpaceLow { reason });
        }
    }

    fn clear_disk_block(&self) {
        if self.disk_blocked.swap(false, Ordering::SeqCst) {
            if let Ok(mut current) = self.disk_block_reason.lock() {
                *current = None;
            }
            info!("Disk guardrail cleared — resuming job starts.");
            let _ = self
                .event_channels
                .system
                .send(SystemEvent::EngineStatusChanged);
        }
    }

    /// AUTO-3: returns `true` when the engine should hold this iteration because
    /// the next queued job's output filesystem has less than
    /// `system.min_free_space_gb` free. Fails open — a disabled guardrail, no
    /// queued job, or an undeterminable free-space value never holds.
    async fn disk_guardrail_should_hold(&self) -> bool {
        let min_gb = self.config.read().await.system.min_free_space_gb;
        if min_gb == 0 {
            self.clear_disk_block();
            return false;
        }

        let next = match self.db.get_next_job().await {
            Ok(Some(job)) => job,
            Ok(None) => {
                self.clear_disk_block();
                return false;
            }
            Err(e) => {
                // A peek failure is a database problem, not a disk one; let the
                // normal claim path surface it instead of engaging the guard.
                debug!("Disk guardrail: peek failed: {e}");
                return false;
            }
        };

        let output_dir = std::path::Path::new(&next.output_path)
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let available = crate::system::disk_space::available_bytes_for_path(output_dir);

        if crate::system::disk_space::is_below_min_free(available, min_gb) {
            let free_gib = available.map_or(0.0, crate::system::disk_space::as_gib);
            self.engage_disk_block(format!(
                "low disk space on {}: {:.1} GiB free, {} GiB minimum",
                output_dir.display(),
                free_gib,
                min_gb
            ));
            true
        } else {
            self.clear_disk_block();
            false
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

        // Fetch file settings once for the whole scan instead of re-reading
        // them per file, then resolve files up front and write them in chunked
        // transactions instead of one transaction per file. Between chunks we
        // yield so interactive requests (the per-request auth session lookup,
        // jobs list, settings save) can use the connection pool rather than
        // starving behind the scan's writer.
        let settings = self.db.get_file_settings().await.unwrap_or_else(|e| {
            error!("Failed to fetch file settings, using defaults: {}", e);
            crate::media::pipeline::default_file_settings()
        });

        const ENQUEUE_CHUNK: usize = 500;
        let mut buffer: Vec<crate::db::PreparedEnqueue> = Vec::with_capacity(ENQUEUE_CHUNK);
        for scanned_file in files {
            let path = scanned_file.path.clone();
            match crate::media::pipeline::resolve_discovered_for_enqueue(
                &self.db,
                &scanned_file,
                &settings,
            )
            .await
            {
                Ok(Some(prepared)) => {
                    buffer.push(prepared);
                    if buffer.len() >= ENQUEUE_CHUNK {
                        if let Err(e) = self.db.enqueue_jobs_batch(&buffer).await {
                            error!("Failed to enqueue scanned job batch: {}", e);
                        }
                        buffer.clear();
                        tokio::task::yield_now().await;
                    }
                }
                Ok(None) => {}
                Err(e) => error!("Failed to enqueue job for {:?}: {}", path, e),
            }
        }
        if !buffer.is_empty() {
            if let Err(e) = self.db.enqueue_jobs_batch(&buffer).await {
                error!("Failed to enqueue scanned job batch: {}", e);
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
        if self.hardware_state.snapshot().await.is_none() {
            debug!("Auto-analysis: hardware detection pending, skipping pass.");
            return;
        }

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

            // AUTO-3: hold job starts when the next job's output filesystem is
            // below the configured free-space minimum, so we never begin an
            // encode that would fail by filling the disk mid-run. Jobs stay
            // queued and retry once space is reclaimed.
            if self.disk_guardrail_should_hold().await {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                continue;
            }

            if self.hardware_state.snapshot().await.is_none() {
                debug!("Hardware detection pending; engine claim loop is waiting.");
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
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

            let current_mode = *self.engine_mode.read().await;
            match self.db.claim_next_job_with_mode(current_mode).await {
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
                    let job_id = job.id;
                    // Per-job span: every analyze→plan→encode→finalize log line for
                    // this job carries `job_id`, so a failure is traceable end to end.
                    let job_span = tracing::info_span!("job", job_id);
                    tokio::spawn(async move {
                        use tracing::Instrument;
                        struct InFlightGuard(Arc<AtomicUsize>);
                        impl Drop for InFlightGuard {
                            fn drop(&mut self) {
                                self.0.fetch_sub(1, Ordering::SeqCst);
                            }
                        }

                        let _guard = InFlightGuard(counter);
                        let _permit = permit;
                        if let Err(e) = agent.process_job(job).instrument(job_span).await {
                            error!(job_id, "Job processing error: {}", e);
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
        use crate::media::pipeline::JobFailure;

        let job_id = job.id;
        // The pipeline increments attempt_count during the run, so the number of
        // attempts made once it returns is the pre-run count plus one.
        let attempts_made = job.attempt_count.saturating_add(1);
        let pipeline = self.pipeline();

        match pipeline.process_job(job).await {
            Ok(()) => Ok(()),
            Err(failure) => {
                let code = job_failure_code(&failure);
                // Only genuinely transient failures (transient IO, a full disk
                // that may clear) are worth retrying; deterministic failures
                // (corrupt media, encoder-open, planner bug) fail fast. The
                // pipeline already attempts a one-time CPU fallback for hardware
                // encoder-open failures before surfacing them here.
                if matches!(failure, JobFailure::Transient)
                    && attempts_made < MAX_TRANSIENT_ATTEMPTS
                {
                    let backoff = transient_backoff(attempts_made);
                    tracing::warn!(
                        job_id,
                        attempt = attempts_made,
                        max_attempts = MAX_TRANSIENT_ATTEMPTS,
                        error_code = code,
                        "Transient job failure; requeueing in {}s",
                        backoff.as_secs()
                    );
                    tokio::time::sleep(backoff).await;
                    if let Err(e) = self
                        .db
                        .update_job_status(job_id, crate::db::JobState::Queued)
                        .await
                    {
                        tracing::error!(job_id, "Failed to requeue transient job: {e}");
                        return Err(crate::error::AlchemistError::Unknown(format!(
                            "job {job_id} requeue failed after transient error: {e}"
                        )));
                    }
                    return Ok(());
                }

                // Surface a coded, legible error instead of an opaque
                // "Unknown error: Transient" (see errors#{code}).
                Err(crate::error::AlchemistError::Unknown(format!(
                    "job {job_id} failed ({code})"
                )))
            }
        }
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

/// Maximum total encode attempts (including the first) for a transient failure
/// before the job is left in the Failed state.
const MAX_TRANSIENT_ATTEMPTS: i32 = 3;

/// Capped exponential backoff before requeueing a transient failure. Holding the
/// concurrency permit during this sleep naturally throttles a systemic problem
/// (e.g. a full disk) instead of hot-looping.
fn transient_backoff(attempt: i32) -> tokio::time::Duration {
    let secs = 2u64.saturating_pow(attempt.clamp(1, 4) as u32).min(30);
    tokio::time::Duration::from_secs(secs)
}

/// Stable code for a `JobFailure`, aligned with the docs error reference
/// (`errors#<code>`) so log lines and the failure surface agree.
fn job_failure_code(failure: &crate::media::pipeline::JobFailure) -> &'static str {
    use crate::media::pipeline::JobFailure;
    match failure {
        JobFailure::Transient => "transient",
        JobFailure::MediaCorrupt => "corrupt_or_unreadable_media",
        JobFailure::EncoderUnavailable => "encoder_unavailable",
        JobFailure::PlannerBug => "planning_failed",
    }
}

#[cfg(test)]
mod retry_tests {
    use super::*;

    #[test]
    fn transient_backoff_is_bounded_and_increasing() {
        assert_eq!(transient_backoff(1).as_secs(), 2);
        assert_eq!(transient_backoff(2).as_secs(), 4);
        assert!(transient_backoff(10).as_secs() <= 30);
    }

    #[test]
    fn job_failure_codes_match_docs_reference() {
        use crate::media::pipeline::JobFailure;
        assert_eq!(job_failure_code(&JobFailure::Transient), "transient");
        assert_eq!(
            job_failure_code(&JobFailure::EncoderUnavailable),
            "encoder_unavailable"
        );
    }
}
