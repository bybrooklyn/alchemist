use std::collections::VecDeque;
use std::fmt;

/// Policy for how the scheduler manages concurrent transcoding jobs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerPolicy {
    /// Maximum number of simultaneous jobs.
    pub max_simultaneous_jobs: usize,
    /// Number of CPU worker threads (0 = auto-detect).
    pub cpu_workers: usize,
    /// Whether to use chunked parallel encoding within a single job.
    pub chunked_encoding: bool,
    /// Maximum chunks per file when chunking is enabled.
    pub max_chunks_per_file: usize,
}

impl Default for SchedulerPolicy {
    fn default() -> Self {
        Self {
            max_simultaneous_jobs: 3,
            cpu_workers: 0,
            chunked_encoding: true,
            max_chunks_per_file: 8,
        }
    }
}

impl SchedulerPolicy {
    /// Resolve the actual worker count: use configured value, or auto-detect
    /// from available parallelism.
    pub fn effective_workers(&self) -> usize {
        if self.cpu_workers > 0 {
            self.cpu_workers
        } else {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        }
    }
}

impl fmt::Display for SchedulerPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "jobs={}, workers={}, chunking={}",
            self.max_simultaneous_jobs,
            if self.cpu_workers == 0 {
                "auto".to_string()
            } else {
                self.cpu_workers.to_string()
            },
            if self.chunked_encoding { "on" } else { "off" }
        )
    }
}

/// Priority of a queued job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobPriority {
    Low,
    Normal,
    High,
    Urgent,
}

impl Default for JobPriority {
    fn default() -> Self {
        Self::Normal
    }
}

impl fmt::Display for JobPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Urgent => write!(f, "urgent"),
        }
    }
}

/// Status of a scheduled job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Complete,
    Failed,
    Cancelled,
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Running => write!(f, "running"),
            Self::Complete => write!(f, "complete"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A job in the scheduler queue.
#[derive(Debug, Clone)]
pub struct ScheduledJob {
    pub id: u64,
    pub input_path: String,
    pub output_path: String,
    pub priority: JobPriority,
    pub status: JobStatus,
}

/// Manages a queue of transcoding jobs with priority ordering.
#[derive(Debug)]
pub struct Scheduler {
    policy: SchedulerPolicy,
    queue: VecDeque<ScheduledJob>,
    next_id: u64,
    running_count: usize,
}

impl Scheduler {
    pub fn new(policy: SchedulerPolicy) -> Self {
        Self {
            policy,
            queue: VecDeque::new(),
            next_id: 1,
            running_count: 0,
        }
    }

    pub fn policy(&self) -> &SchedulerPolicy {
        &self.policy
    }

    /// Add a job to the queue. Returns the assigned job ID.
    pub fn enqueue(&mut self, input: &str, output: &str, priority: JobPriority) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let job = ScheduledJob {
            id,
            input_path: input.to_string(),
            output_path: output.to_string(),
            priority,
            status: JobStatus::Queued,
        };
        // Insert maintaining priority order (highest first)
        let pos = self
            .queue
            .iter()
            .position(|j| j.priority < priority)
            .unwrap_or(self.queue.len());
        self.queue.insert(pos, job);
        id
    }

    /// Claim the next available job if capacity allows.
    pub fn claim_next(&mut self) -> Option<&mut ScheduledJob> {
        if self.running_count >= self.policy.max_simultaneous_jobs {
            return None;
        }
        let idx = self
            .queue
            .iter()
            .position(|j| j.status == JobStatus::Queued)?;
        self.queue[idx].status = JobStatus::Running;
        self.running_count += 1;
        Some(&mut self.queue[idx])
    }

    /// Mark a job as complete.
    pub fn complete(&mut self, id: u64) {
        if let Some(job) = self.queue.iter_mut().find(|j| j.id == id) {
            if job.status == JobStatus::Running {
                self.running_count = self.running_count.saturating_sub(1);
            }
            job.status = JobStatus::Complete;
        }
    }

    /// Mark a job as failed.
    pub fn fail(&mut self, id: u64) {
        if let Some(job) = self.queue.iter_mut().find(|j| j.id == id) {
            if job.status == JobStatus::Running {
                self.running_count = self.running_count.saturating_sub(1);
            }
            job.status = JobStatus::Failed;
        }
    }

    /// Cancel a queued or running job.
    pub fn cancel(&mut self, id: u64) {
        if let Some(job) = self.queue.iter_mut().find(|j| j.id == id) {
            if job.status == JobStatus::Running {
                self.running_count = self.running_count.saturating_sub(1);
            }
            job.status = JobStatus::Cancelled;
        }
    }

    /// Number of jobs currently running.
    pub fn running_count(&self) -> usize {
        self.running_count
    }

    /// Number of jobs waiting in queue.
    pub fn queued_count(&self) -> usize {
        self.queue
            .iter()
            .filter(|j| j.status == JobStatus::Queued)
            .count()
    }

    /// Total jobs (all statuses).
    pub fn total_count(&self) -> usize {
        self.queue.len()
    }

    /// Get a job by ID.
    pub fn get(&self, id: u64) -> Option<&ScheduledJob> {
        self.queue.iter().find(|j| j.id == id)
    }

    /// Remove completed/cancelled/failed jobs from the queue.
    pub fn clear_finished(&mut self) {
        self.queue
            .retain(|j| matches!(j.status, JobStatus::Queued | JobStatus::Running));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_policy() -> SchedulerPolicy {
        SchedulerPolicy {
            max_simultaneous_jobs: 2,
            cpu_workers: 4,
            chunked_encoding: false,
            max_chunks_per_file: 1,
        }
    }

    #[test]
    fn enqueue_returns_sequential_ids() {
        let mut sched = Scheduler::new(test_policy());
        let id1 = sched.enqueue("a.mkv", "a.out.mkv", JobPriority::Normal);
        let id2 = sched.enqueue("b.mkv", "b.out.mkv", JobPriority::Normal);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(sched.total_count(), 2);
    }

    #[test]
    fn priority_ordering() {
        let mut sched = Scheduler::new(test_policy());
        sched.enqueue("low.mkv", "out.mkv", JobPriority::Low);
        sched.enqueue("urgent.mkv", "out.mkv", JobPriority::Urgent);
        sched.enqueue("normal.mkv", "out.mkv", JobPriority::Normal);

        let first = sched.claim_next().unwrap();
        assert_eq!(first.input_path, "urgent.mkv");
    }

    #[test]
    fn claim_respects_max_jobs() {
        let mut sched = Scheduler::new(test_policy());
        sched.enqueue("a.mkv", "out.mkv", JobPriority::Normal);
        sched.enqueue("b.mkv", "out.mkv", JobPriority::Normal);
        sched.enqueue("c.mkv", "out.mkv", JobPriority::Normal);

        sched.claim_next();
        sched.claim_next();
        assert!(sched.claim_next().is_none()); // max 2
        assert_eq!(sched.running_count(), 2);
    }

    #[test]
    fn complete_frees_capacity() {
        let mut sched = Scheduler::new(test_policy());
        let id = sched.enqueue("a.mkv", "out.mkv", JobPriority::Normal);
        sched.claim_next();
        sched.complete(id);
        assert_eq!(sched.running_count(), 0);
        assert_eq!(sched.queued_count(), 0);
    }

    #[test]
    fn cancel_queued_job() {
        let mut sched = Scheduler::new(test_policy());
        let id = sched.enqueue("a.mkv", "out.mkv", JobPriority::Normal);
        sched.cancel(id);
        assert_eq!(sched.queued_count(), 0);
        assert!(sched.claim_next().is_none());
    }

    #[test]
    fn clear_finished_removes_done() {
        let mut sched = Scheduler::new(test_policy());
        let id1 = sched.enqueue("a.mkv", "out.mkv", JobPriority::Normal);
        sched.enqueue("b.mkv", "out.mkv", JobPriority::Normal);
        sched.claim_next();
        sched.complete(id1);
        sched.clear_finished();
        assert_eq!(sched.total_count(), 1);
    }

    #[test]
    fn effective_workers_auto() {
        let policy = SchedulerPolicy {
            cpu_workers: 0,
            ..Default::default()
        };
        assert!(policy.effective_workers() >= 1);
    }

    #[test]
    fn effective_workers_explicit() {
        let policy = SchedulerPolicy {
            cpu_workers: 8,
            ..Default::default()
        };
        assert_eq!(policy.effective_workers(), 8);
    }
}
