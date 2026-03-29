use crate::error::{AlchemistError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::timeout;
use tracing::info;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Analyzing,
    Encoding,
    Remuxing,
    Completed,
    Skipped,
    Failed,
    Cancelled,
    Resuming,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct JobStats {
    pub active: i64,
    pub queued: i64,
    pub completed: i64,
    pub failed: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LogEntry {
    pub id: i64,
    pub level: String,
    pub job_id: Option<i64>,
    pub message: String,
    pub created_at: String, // SQLite datetime as string
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AlchemistEvent {
    JobStateChanged {
        job_id: i64,
        status: JobState,
    },
    Progress {
        job_id: i64,
        percentage: f64,
        time: String,
    },
    Decision {
        job_id: i64,
        action: String,
        reason: String,
    },
    Log {
        level: String,
        job_id: Option<i64>,
        message: String,
    },
}

// New typed event channels for separating high-volume vs low-volume events
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum JobEvent {
    StateChanged {
        job_id: i64,
        status: JobState,
    },
    Progress {
        job_id: i64,
        percentage: f64,
        time: String,
    },
    Decision {
        job_id: i64,
        action: String,
        reason: String,
    },
    Log {
        level: String,
        job_id: Option<i64>,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ConfigEvent {
    Updated(Box<crate::config::Config>),
    WatchFolderAdded(String),
    WatchFolderRemoved(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SystemEvent {
    ScanStarted,
    ScanCompleted,
    EngineStatusChanged,
    HardwareStateChanged,
}

pub struct EventChannels {
    pub jobs: tokio::sync::broadcast::Sender<JobEvent>, // 1000 capacity - high volume
    pub config: tokio::sync::broadcast::Sender<ConfigEvent>, // 50 capacity - rare
    pub system: tokio::sync::broadcast::Sender<SystemEvent>, // 100 capacity - medium
}

// Convert JobEvent to legacy AlchemistEvent for backwards compatibility
impl From<JobEvent> for AlchemistEvent {
    fn from(job_event: JobEvent) -> Self {
        match job_event {
            JobEvent::StateChanged { job_id, status } => {
                AlchemistEvent::JobStateChanged { job_id, status }
            }
            JobEvent::Progress {
                job_id,
                percentage,
                time,
            } => AlchemistEvent::Progress {
                job_id,
                percentage,
                time,
            },
            JobEvent::Decision {
                job_id,
                action,
                reason,
            } => AlchemistEvent::Decision {
                job_id,
                action,
                reason,
            },
            JobEvent::Log {
                level,
                job_id,
                message,
            } => AlchemistEvent::Log {
                level,
                job_id,
                message,
            },
        }
    }
}

// Convert AlchemistEvent to JobEvent for migration
impl From<AlchemistEvent> for JobEvent {
    fn from(alchemist_event: AlchemistEvent) -> Self {
        match alchemist_event {
            AlchemistEvent::JobStateChanged { job_id, status } => {
                JobEvent::StateChanged { job_id, status }
            }
            AlchemistEvent::Progress {
                job_id,
                percentage,
                time,
            } => JobEvent::Progress {
                job_id,
                percentage,
                time,
            },
            AlchemistEvent::Decision {
                job_id,
                action,
                reason,
            } => JobEvent::Decision {
                job_id,
                action,
                reason,
            },
            AlchemistEvent::Log {
                level,
                job_id,
                message,
            } => JobEvent::Log {
                level,
                job_id,
                message,
            },
        }
    }
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            JobState::Queued => "queued",
            JobState::Analyzing => "analyzing",
            JobState::Encoding => "encoding",
            JobState::Remuxing => "remuxing",
            JobState::Completed => "completed",
            JobState::Skipped => "skipped",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
            JobState::Resuming => "resuming",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Job {
    pub id: i64,
    pub input_path: String,
    pub output_path: String,
    pub status: JobState,
    pub decision_reason: Option<String>,
    pub priority: i32,
    pub progress: f64,
    pub attempt_count: i32,
    pub vmaf_score: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct JobWithHealthIssueRow {
    pub id: i64,
    pub input_path: String,
    pub output_path: String,
    pub status: JobState,
    pub decision_reason: Option<String>,
    pub priority: i32,
    pub progress: f64,
    pub attempt_count: i32,
    pub vmaf_score: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub health_issues: String,
}

impl JobWithHealthIssueRow {
    pub fn into_parts(self) -> (Job, String) {
        (
            Job {
                id: self.id,
                input_path: self.input_path,
                output_path: self.output_path,
                status: self.status,
                decision_reason: self.decision_reason,
                priority: self.priority,
                progress: self.progress,
                attempt_count: self.attempt_count,
                vmaf_score: self.vmaf_score,
                created_at: self.created_at,
                updated_at: self.updated_at,
            },
            self.health_issues,
        )
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct DuplicateCandidate {
    pub id: i64,
    pub input_path: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct WatchDir {
    pub id: i64,
    pub path: String,
    pub is_recursive: bool,
    pub profile_id: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct LibraryProfile {
    pub id: i64,
    pub name: String,
    pub preset: String,
    pub codec: String,
    pub quality_profile: String,
    pub hdr_mode: String,
    pub audio_mode: String,
    pub crf_override: Option<i32>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NewLibraryProfile {
    pub name: String,
    pub preset: String,
    pub codec: String,
    pub quality_profile: String,
    pub hdr_mode: String,
    pub audio_mode: String,
    pub crf_override: Option<i32>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct JobFilterQuery {
    pub limit: i64,
    pub offset: i64,
    pub statuses: Option<Vec<JobState>>,
    pub search: Option<String>,
    pub sort_by: Option<String>,
    pub sort_desc: bool,
    pub archived: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct NotificationTarget {
    pub id: i64,
    pub name: String,
    pub target_type: String,
    pub endpoint_url: String,
    pub auth_token: Option<String>,
    pub events: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct ScheduleWindow {
    pub id: i64,
    pub start_time: String,
    pub end_time: String,
    pub days_of_week: String, // as JSON string
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct FileSettings {
    pub id: i64,
    pub delete_source: bool,
    pub output_extension: String,
    pub output_suffix: String,
    pub replace_strategy: String,
    pub output_root: Option<String>,
}

impl FileSettings {
    pub fn output_path_for(&self, input_path: &Path) -> PathBuf {
        self.output_path_for_source(input_path, None)
    }

    pub fn output_path_for_source(&self, input_path: &Path, source_root: Option<&Path>) -> PathBuf {
        let mut output_path = self.output_base_path(input_path, source_root);
        let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
        let extension = self.output_extension.trim_start_matches('.');
        let suffix = self.output_suffix.as_str();

        let mut filename = String::new();
        filename.push_str(&stem);
        filename.push_str(suffix);
        if !extension.is_empty() {
            filename.push('.');
            filename.push_str(extension);
        }
        if filename.is_empty() {
            filename.push_str("output");
        }
        output_path.set_file_name(filename);

        if output_path == input_path {
            let safe_suffix = if suffix.is_empty() {
                "-alchemist".to_string()
            } else {
                format!("{}-alchemist", suffix)
            };
            let mut safe_name = String::new();
            safe_name.push_str(&stem);
            safe_name.push_str(&safe_suffix);
            if !extension.is_empty() {
                safe_name.push('.');
                safe_name.push_str(extension);
            }
            output_path.set_file_name(safe_name);
        }

        output_path
    }

    fn output_base_path(&self, input_path: &Path, source_root: Option<&Path>) -> PathBuf {
        let Some(output_root) = self
            .output_root
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        else {
            return input_path.to_path_buf();
        };

        let Some(root) = source_root else {
            return input_path.to_path_buf();
        };

        let Ok(relative_path) = input_path.strip_prefix(root) else {
            return input_path.to_path_buf();
        };

        let mut output_path = PathBuf::from(output_root);
        if let Some(parent) = relative_path.parent() {
            output_path.push(parent);
        }
        output_path.push(relative_path.file_name().unwrap_or_default());
        output_path
    }

    pub fn should_replace_existing_output(&self) -> bool {
        let strategy = self.replace_strategy.trim();
        strategy.eq_ignore_ascii_case("replace") || strategy.eq_ignore_ascii_case("overwrite")
    }
}

impl Job {
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            JobState::Encoding | JobState::Analyzing | JobState::Remuxing | JobState::Resuming
        )
    }

    pub fn can_retry(&self) -> bool {
        matches!(self.status, JobState::Failed | JobState::Cancelled)
    }

    pub fn status_class(&self) -> &'static str {
        match self.status {
            JobState::Completed => "badge-green",
            JobState::Encoding | JobState::Remuxing | JobState::Resuming => "badge-yellow",
            JobState::Analyzing => "badge-blue",
            JobState::Failed | JobState::Cancelled => "badge-red",
            _ => "badge-gray",
        }
    }

    pub fn progress_fixed(&self) -> String {
        format!("{:.1}", self.progress)
    }

    pub fn vmaf_fixed(&self) -> String {
        self.vmaf_score
            .map(|s| format!("{:.1}", s))
            .unwrap_or_else(|| "N/A".to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct AggregatedStats {
    pub total_jobs: i64,
    pub completed_jobs: i64,
    pub total_input_size: i64,
    pub total_output_size: i64,
    pub avg_vmaf: Option<f64>,
    pub total_encode_time_seconds: f64,
}

impl AggregatedStats {
    pub fn total_savings_gb(&self) -> f64 {
        (self.total_input_size - self.total_output_size).max(0) as f64 / 1_073_741_824.0
    }

    pub fn total_input_gb(&self) -> f64 {
        self.total_input_size as f64 / 1_073_741_824.0
    }

    pub fn avg_reduction_percentage(&self) -> f64 {
        if self.total_input_size == 0 {
            0.0
        } else {
            (1.0 - (self.total_output_size as f64 / self.total_input_size as f64)) * 100.0
        }
    }

    pub fn total_time_hours(&self) -> f64 {
        self.total_encode_time_seconds / 3600.0
    }

    pub fn total_savings_fixed(&self) -> String {
        format!("{:.1}", self.total_savings_gb())
    }

    pub fn total_input_fixed(&self) -> String {
        format!("{:.1}", self.total_input_gb())
    }

    pub fn efficiency_fixed(&self) -> String {
        format!("{:.1}", self.avg_reduction_percentage())
    }

    pub fn time_fixed(&self) -> String {
        format!("{:.1}", self.total_time_hours())
    }

    pub fn avg_vmaf_fixed(&self) -> String {
        self.avg_vmaf
            .map(|v| format!("{:.1}", v))
            .unwrap_or_else(|| "N/A".to_string())
    }
}

/// Daily aggregated statistics for time-series charts
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DailyStats {
    pub date: String,
    pub jobs_completed: i64,
    pub bytes_saved: i64,
    pub total_input_bytes: i64,
    pub total_output_bytes: i64,
}

/// Detailed per-job encoding statistics
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct DetailedEncodeStats {
    pub job_id: i64,
    pub input_path: String,
    pub input_size_bytes: i64,
    pub output_size_bytes: i64,
    pub compression_ratio: f64,
    pub encode_time_seconds: f64,
    pub encode_speed: f64,
    pub avg_bitrate_kbps: f64,
    pub vmaf_score: Option<f64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct EncodeStatsInput {
    pub job_id: i64,
    pub input_size: u64,
    pub output_size: u64,
    pub compression_ratio: f64,
    pub encode_time: f64,
    pub encode_speed: f64,
    pub avg_bitrate: f64,
    pub vmaf_score: Option<f64>,
    pub output_codec: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodecSavings {
    pub codec: String,
    pub bytes_saved: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DailySavings {
    pub date: String,
    pub bytes_saved: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SavingsSummary {
    pub total_input_bytes: i64,
    pub total_output_bytes: i64,
    pub total_bytes_saved: i64,
    pub savings_percent: f64,
    pub job_count: i64,
    pub savings_by_codec: Vec<CodecSavings>,
    pub savings_over_time: Vec<DailySavings>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HealthSummary {
    pub total_checked: i64,
    pub issues_found: i64,
    pub last_run: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Decision {
    pub id: i64,
    pub job_id: i64,
    pub action: String, // "encode", "skip", "reject"
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

/// Default timeout for potentially slow database queries
const QUERY_TIMEOUT: Duration = Duration::from_secs(5);

/// Execute a query with a timeout to prevent blocking the job loop
async fn timed_query<T, F, Fut>(operation: &str, f: F) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    match timeout(QUERY_TIMEOUT, f()).await {
        Ok(result) => result,
        Err(_) => Err(AlchemistError::QueryTimeout(
            QUERY_TIMEOUT.as_secs(),
            operation.to_string(),
        )),
    }
}

#[derive(Clone, Debug)]
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn new(db_path: &str) -> Result<Self> {
        let start = std::time::Instant::now();
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePool::connect_with(options).await?;
        info!(
            target: "startup",
            "Database connection opened in {} ms",
            start.elapsed().as_millis()
        );

        // Run migrations
        let migrate_start = std::time::Instant::now();
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| crate::error::AlchemistError::Database(e.into()))?;
        info!(
            target: "startup",
            "Database migrations completed in {} ms",
            migrate_start.elapsed().as_millis()
        );

        Ok(Self { pool })
    }

    // init method removed as it is replaced by migrations

    pub async fn reset_interrupted_jobs(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE jobs
             SET status = 'queued',
                 progress = 0.0,
                 updated_at = CURRENT_TIMESTAMP
             WHERE status IN ('encoding', 'analyzing', 'remuxing') AND archived = 0",
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn enqueue_job(
        &self,
        input_path: &Path,
        output_path: &Path,
        mtime: std::time::SystemTime,
    ) -> Result<bool> {
        if input_path == output_path {
            return Err(crate::error::AlchemistError::Config(
                "Output path matches input path".into(),
            ));
        }
        let input_str = input_path
            .to_str()
            .ok_or_else(|| crate::error::AlchemistError::Config("Invalid input path".into()))?;
        let output_str = output_path
            .to_str()
            .ok_or_else(|| crate::error::AlchemistError::Config("Invalid output path".into()))?;

        // Stable mtime representation (seconds + nanos)
        let mtime_hash = match mtime.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => format!("{}.{:09}", d.as_secs(), d.subsec_nanos()),
            Err(_) => "0.0".to_string(), // Fallback for very old files/clocks
        };

        let result = sqlx::query(
            "INSERT INTO jobs (input_path, output_path, status, mtime_hash, updated_at) 
             VALUES (?, ?, 'queued', ?, CURRENT_TIMESTAMP)
             ON CONFLICT(input_path) DO UPDATE SET
             output_path = excluded.output_path,
             status = CASE WHEN mtime_hash != excluded.mtime_hash THEN 'queued' ELSE status END,
             archived = 0,
             mtime_hash = excluded.mtime_hash,
             updated_at = CURRENT_TIMESTAMP
             WHERE mtime_hash != excluded.mtime_hash OR output_path != excluded.output_path",
        )
        .bind(input_str)
        .bind(output_str)
        .bind(mtime_hash)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn add_job(&self, job: Job) -> Result<()> {
        sqlx::query(
            "INSERT INTO jobs (input_path, output_path, status, mtime_hash, priority, progress, attempt_count, created_at, updated_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(job.input_path)
        .bind(job.output_path)
        .bind(job.status)
        .bind("0.0")
        .bind(job.priority)
        .bind(job.progress)
        .bind(job.attempt_count)
        .bind(job.created_at)
        .bind(job.updated_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_next_job(&self) -> Result<Option<Job>> {
        let job = sqlx::query_as::<_, Job>(
            "SELECT id, input_path, output_path, status, NULL as decision_reason,
                    COALESCE(priority, 0) as priority, COALESCE(CAST(progress AS REAL), 0.0) as progress,
                    COALESCE(attempt_count, 0) as attempt_count,
                    NULL as vmaf_score,
                    created_at, updated_at 
             FROM jobs
             WHERE status = 'queued'
               AND archived = 0
               AND (
                    COALESCE(attempt_count, 0) = 0
                    OR CASE
                        WHEN COALESCE(attempt_count, 0) = 1 THEN datetime(updated_at, '+5 minutes')
                        WHEN COALESCE(attempt_count, 0) = 2 THEN datetime(updated_at, '+15 minutes')
                        WHEN COALESCE(attempt_count, 0) = 3 THEN datetime(updated_at, '+60 minutes')
                        ELSE datetime(updated_at, '+360 minutes')
                    END <= datetime('now')
               )
             ORDER BY priority DESC, created_at ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(job)
    }

    pub async fn claim_next_job(&self) -> Result<Option<Job>> {
        let job = sqlx::query_as::<_, Job>(
            "UPDATE jobs
             SET status = 'analyzing', updated_at = CURRENT_TIMESTAMP
             WHERE id = (
                 SELECT id
                 FROM jobs
                 WHERE status = 'queued'
                   AND archived = 0
                   AND (
                        COALESCE(attempt_count, 0) = 0
                        OR CASE
                            WHEN COALESCE(attempt_count, 0) = 1 THEN datetime(updated_at, '+5 minutes')
                            WHEN COALESCE(attempt_count, 0) = 2 THEN datetime(updated_at, '+15 minutes')
                            WHEN COALESCE(attempt_count, 0) = 3 THEN datetime(updated_at, '+60 minutes')
                            ELSE datetime(updated_at, '+360 minutes')
                        END <= datetime('now')
                   )
                 ORDER BY priority DESC, created_at ASC LIMIT 1
             )
             RETURNING id, input_path, output_path, status, NULL as decision_reason,
                       COALESCE(priority, 0) as priority, COALESCE(CAST(progress AS REAL), 0.0) as progress,
                       COALESCE(attempt_count, 0) as attempt_count,
                       NULL as vmaf_score,
                       created_at, updated_at",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(job)
    }

    pub async fn update_job_status(&self, id: i64, status: JobState) -> Result<()> {
        let result =
            sqlx::query("UPDATE jobs SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(status)
                .bind(id)
                .execute(&self.pool)
                .await?;

        if result.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }

        Ok(())
    }

    pub async fn add_decision(&self, job_id: i64, action: &str, reason: &str) -> Result<()> {
        sqlx::query("INSERT INTO decisions (job_id, action, reason) VALUES (?, ?, ?)")
            .bind(job_id)
            .bind(action)
            .bind(reason)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_all_jobs(&self) -> Result<Vec<Job>> {
        let pool = &self.pool;
        timed_query("get_all_jobs", || async {
            let jobs = sqlx::query_as::<_, Job>(
                "SELECT j.id, j.input_path, j.output_path, j.status, 
                        (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                        COALESCE(j.priority, 0) as priority, 
                        COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                        COALESCE(j.attempt_count, 0) as attempt_count,
                        (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                        j.created_at, j.updated_at
                 FROM jobs j
                 WHERE j.archived = 0
                 ORDER BY j.updated_at DESC",
            )
            .fetch_all(pool)
            .await?;

            Ok(jobs)
        })
        .await
    }

    /// Returns all jobs whose filename stem appears more than
    /// once across the library. Groups by stem, filtered to
    /// only non-cancelled jobs. Grouping and path parsing is
    /// done in Rust using std::path::Path.
    pub async fn get_duplicate_candidates(&self) -> Result<Vec<DuplicateCandidate>> {
        timed_query("get_duplicate_candidates", || async {
            let rows: Vec<DuplicateCandidate> = sqlx::query_as(
                "SELECT id, input_path, status FROM jobs
                 WHERE status NOT IN ('cancelled')
                 ORDER BY input_path ASC",
            )
            .fetch_all(&self.pool)
            .await?;
            Ok(rows)
        })
        .await
    }

    pub async fn get_job_decision(&self, job_id: i64) -> Result<Option<Decision>> {
        let decision = sqlx::query_as::<_, Decision>(
            "SELECT id, job_id, action, reason, created_at FROM decisions WHERE job_id = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(decision)
    }

    pub async fn get_stats(&self) -> Result<serde_json::Value> {
        let pool = &self.pool;
        timed_query("get_stats", || async {
            let stats = sqlx::query("SELECT status, count(*) as count FROM jobs GROUP BY status")
                .fetch_all(pool)
                .await?;

            let mut map = serde_json::Map::new();
            for row in stats {
                use sqlx::Row;
                let status: String = row.get("status");
                let count: i64 = row.get("count");
                map.insert(status, serde_json::Value::Number(count.into()));
            }

            Ok(serde_json::Value::Object(map))
        })
        .await
    }

    /// Update job progress (for resume support)
    pub async fn update_job_progress(&self, id: i64, progress: f64) -> Result<()> {
        let result = sqlx::query(
            "UPDATE jobs SET progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(progress)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }

        Ok(())
    }

    /// Set job priority
    pub async fn set_job_priority(&self, id: i64, priority: i32) -> Result<()> {
        let result = sqlx::query(
            "UPDATE jobs SET priority = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(priority)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }

        Ok(())
    }

    /// Save encode statistics
    pub async fn save_encode_stats(&self, stats: EncodeStatsInput) -> Result<()> {
        let result = sqlx::query(
            "INSERT INTO encode_stats 
             (job_id, input_size_bytes, output_size_bytes, compression_ratio, 
              encode_time_seconds, encode_speed, avg_bitrate_kbps, vmaf_score, output_codec)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(job_id) DO UPDATE SET
             input_size_bytes = excluded.input_size_bytes,
             output_size_bytes = excluded.output_size_bytes,
             compression_ratio = excluded.compression_ratio,
             encode_time_seconds = excluded.encode_time_seconds,
             encode_speed = excluded.encode_speed,
             avg_bitrate_kbps = excluded.avg_bitrate_kbps,
             vmaf_score = excluded.vmaf_score,
             output_codec = excluded.output_codec",
        )
        .bind(stats.job_id)
        .bind(stats.input_size as i64)
        .bind(stats.output_size as i64)
        .bind(stats.compression_ratio)
        .bind(stats.encode_time)
        .bind(stats.encode_speed)
        .bind(stats.avg_bitrate)
        .bind(stats.vmaf_score)
        .bind(stats.output_codec)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }

        Ok(())
    }

    /// Get job by ID
    pub async fn get_job(&self, id: i64) -> Result<Option<Job>> {
        let job = sqlx::query_as::<_, Job>(
            "SELECT j.id, j.input_path, j.output_path, j.status, 
                    (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                    COALESCE(j.priority, 0) as priority, 
                    COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                    j.created_at, j.updated_at
             FROM jobs j
             WHERE j.id = ? AND j.archived = 0",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(job)
    }

    /// Get jobs by status
    pub async fn get_jobs_by_status(&self, status: JobState) -> Result<Vec<Job>> {
        let pool = &self.pool;
        timed_query("get_jobs_by_status", || async {
            let jobs = sqlx::query_as::<_, Job>(
                "SELECT j.id, j.input_path, j.output_path, j.status, 
                        (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                        COALESCE(j.priority, 0) as priority, 
                        COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                        COALESCE(j.attempt_count, 0) as attempt_count,
                        (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                        j.created_at, j.updated_at
                 FROM jobs j
                 WHERE j.status = ? AND j.archived = 0
                 ORDER BY j.priority DESC, j.created_at ASC",
            )
            .bind(status)
            .fetch_all(pool)
            .await?;

            Ok(jobs)
        })
        .await
    }

    /// Get jobs with filtering, sorting and pagination
    pub async fn get_jobs_filtered(&self, query: JobFilterQuery) -> Result<Vec<Job>> {
        let pool = &self.pool;
        timed_query("get_jobs_filtered", || async {
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "SELECT j.id, j.input_path, j.output_path, j.status, 
                        (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                        COALESCE(j.priority, 0) as priority, 
                        COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                        COALESCE(j.attempt_count, 0) as attempt_count,
                        (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                        j.created_at, j.updated_at
                 FROM jobs j
                 WHERE 1 = 1 "
            );

            match query.archived {
                Some(true) => {
                    qb.push(" AND j.archived = 1 ");
                }
                Some(false) => {
                    qb.push(" AND j.archived = 0 ");
                }
                None => {}
            }

            if let Some(ref statuses) = query.statuses {
                if !statuses.is_empty() {
                    qb.push(" AND j.status IN (");
                    let mut separated = qb.separated(", ");
                    for status in statuses {
                        separated.push_bind(*status);
                    }
                    separated.push_unseparated(") ");
                }
            }

            if let Some(ref search) = query.search {
                qb.push(" AND j.input_path LIKE ");
                qb.push_bind(format!("%{}%", search));
            }

            qb.push(" ORDER BY ");
            let sort_col = match query.sort_by.as_deref() {
                Some("created_at") => "j.created_at",
                Some("updated_at") => "j.updated_at",
                Some("input_path") => "j.input_path",
                Some("size") => "(SELECT input_size_bytes FROM encode_stats WHERE job_id = j.id)",
                _ => "j.updated_at",
            };
            qb.push(sort_col);
            qb.push(if query.sort_desc { " DESC" } else { " ASC" });

            qb.push(" LIMIT ");
            qb.push_bind(query.limit);
            qb.push(" OFFSET ");
            qb.push_bind(query.offset);

            let jobs = qb.build_query_as::<Job>().fetch_all(pool).await?;
            Ok(jobs)
        })
        .await
    }

    pub async fn batch_cancel_jobs(&self, ids: &[i64]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "UPDATE jobs SET status = 'cancelled', updated_at = CURRENT_TIMESTAMP WHERE id IN (",
        );
        let mut separated = qb.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let result = qb.build().execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn batch_delete_jobs(&self, ids: &[i64]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "UPDATE jobs SET archived = 1, updated_at = CURRENT_TIMESTAMP WHERE id IN (",
        );
        let mut separated = qb.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let result = qb.build().execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn batch_restart_jobs(&self, ids: &[i64]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "UPDATE jobs SET status = 'queued', progress = 0.0, updated_at = CURRENT_TIMESTAMP WHERE id IN (",
        );
        let mut separated = qb.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let result = qb.build().execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn get_job_by_id(&self, id: i64) -> Result<Option<Job>> {
        let job = sqlx::query_as::<_, Job>(
            "SELECT j.id, j.input_path, j.output_path, j.status, 
                    (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                    COALESCE(j.priority, 0) as priority, 
                    COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                    j.created_at, j.updated_at
             FROM jobs j
             WHERE j.id = ? AND j.archived = 0",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(job)
    }

    /// Returns all jobs in queued or failed state that need
    /// analysis. Used by the startup auto-analyzer.
    pub async fn get_jobs_for_analysis(&self) -> Result<Vec<Job>> {
        timed_query("get_jobs_for_analysis", || async {
            let rows: Vec<Job> = sqlx::query_as(
                "SELECT j.id, j.input_path, j.output_path, j.status, 
                        (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                        COALESCE(j.priority, 0) as priority, 
                        COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                        COALESCE(j.attempt_count, 0) as attempt_count,
                        (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                        j.created_at, j.updated_at
                 FROM jobs j
                 WHERE j.status IN ('queued', 'failed') AND j.archived = 0
                 ORDER BY j.priority DESC, j.created_at ASC",
            )
            .fetch_all(&self.pool)
            .await?;
            Ok(rows)
        })
        .await
    }

    pub async fn get_jobs_by_ids(&self, ids: &[i64]) -> Result<Vec<Job>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT j.id, j.input_path, j.output_path, j.status, 
                    (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                    COALESCE(j.priority, 0) as priority, 
                    COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                    j.created_at, j.updated_at
             FROM jobs j
             WHERE j.archived = 0 AND j.id IN (",
        );
        let mut separated = qb.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        qb.push(" ORDER BY j.updated_at DESC");

        let jobs = qb.build_query_as::<Job>().fetch_all(&self.pool).await?;
        Ok(jobs)
    }

    pub async fn delete_job(&self, id: i64) -> Result<()> {
        let result = sqlx::query(
            "UPDATE jobs
             SET archived = 1, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }
        Ok(())
    }

    pub async fn get_encode_stats_by_job_id(&self, job_id: i64) -> Result<DetailedEncodeStats> {
        let stats = sqlx::query_as::<_, DetailedEncodeStats>(
            "SELECT 
                e.job_id,
                j.input_path,
                e.input_size_bytes,
                e.output_size_bytes,
                e.compression_ratio,
                e.encode_time_seconds,
                e.encode_speed,
                e.avg_bitrate_kbps,
                e.vmaf_score,
                e.created_at
             FROM encode_stats e
             JOIN jobs j ON e.job_id = j.id
             WHERE e.job_id = ?",
        )
        .bind(job_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(stats)
    }

    pub async fn get_watch_dirs(&self) -> Result<Vec<WatchDir>> {
        let has_is_recursive = self.has_column("watch_dirs", "is_recursive").await?;
        let has_recursive = self.has_column("watch_dirs", "recursive").await?;
        let has_enabled = self.has_column("watch_dirs", "enabled").await?;
        let has_profile_id = self.has_column("watch_dirs", "profile_id").await?;

        let recursive_expr = if has_is_recursive {
            "is_recursive"
        } else if has_recursive {
            "recursive"
        } else {
            "1"
        };

        let enabled_filter = if has_enabled {
            "WHERE enabled = 1 "
        } else {
            ""
        };
        let profile_expr = if has_profile_id { "profile_id" } else { "NULL" };
        let query = format!(
            "SELECT id, path, {} as is_recursive, {} as profile_id, created_at
             FROM watch_dirs {}ORDER BY path ASC",
            recursive_expr, profile_expr, enabled_filter
        );

        let dirs = sqlx::query_as::<_, WatchDir>(&query)
            .fetch_all(&self.pool)
            .await?;
        Ok(dirs)
    }

    pub async fn get_job_by_input_path(&self, path: &str) -> Result<Option<Job>> {
        let job = sqlx::query_as::<_, Job>(
            "SELECT j.id, j.input_path, j.output_path, j.status, 
                    (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                    COALESCE(j.priority, 0) as priority, 
                    COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                    j.created_at, j.updated_at
             FROM jobs j
             WHERE j.input_path = ? AND j.archived = 0",
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;

        Ok(job)
    }

    pub async fn has_job_with_output_path(&self, path: &str) -> Result<bool> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM jobs WHERE output_path = ? LIMIT 1")
                .bind(path)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }

    pub async fn add_watch_dir(&self, path: &str, is_recursive: bool) -> Result<WatchDir> {
        let has_is_recursive = self.has_column("watch_dirs", "is_recursive").await?;
        let has_recursive = self.has_column("watch_dirs", "recursive").await?;
        let has_profile_id = self.has_column("watch_dirs", "profile_id").await?;

        let row = if has_is_recursive && has_profile_id {
            sqlx::query_as::<_, WatchDir>(
                "INSERT INTO watch_dirs (path, is_recursive) VALUES (?, ?)
                 RETURNING id, path, is_recursive, profile_id, created_at",
            )
            .bind(path)
            .bind(is_recursive)
            .fetch_one(&self.pool)
            .await?
        } else if has_is_recursive {
            sqlx::query_as::<_, WatchDir>(
                "INSERT INTO watch_dirs (path, is_recursive) VALUES (?, ?)
                 RETURNING id, path, is_recursive, NULL as profile_id, created_at",
            )
            .bind(path)
            .bind(is_recursive)
            .fetch_one(&self.pool)
            .await?
        } else if has_recursive && has_profile_id {
            sqlx::query_as::<_, WatchDir>(
                "INSERT INTO watch_dirs (path, recursive) VALUES (?, ?)
                 RETURNING id, path, recursive as is_recursive, profile_id, created_at",
            )
            .bind(path)
            .bind(is_recursive)
            .fetch_one(&self.pool)
            .await?
        } else if has_recursive {
            sqlx::query_as::<_, WatchDir>(
                "INSERT INTO watch_dirs (path, recursive) VALUES (?, ?)
                 RETURNING id, path, recursive as is_recursive, NULL as profile_id, created_at",
            )
            .bind(path)
            .bind(is_recursive)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, WatchDir>(
                "INSERT INTO watch_dirs (path) VALUES (?)
                 RETURNING id, path, 1 as is_recursive, NULL as profile_id, created_at",
            )
            .bind(path)
            .fetch_one(&self.pool)
            .await?
        };
        Ok(row)
    }

    pub async fn replace_watch_dirs(
        &self,
        watch_dirs: &[crate::config::WatchDirConfig],
    ) -> Result<()> {
        let has_is_recursive = self.has_column("watch_dirs", "is_recursive").await?;
        let has_recursive = self.has_column("watch_dirs", "recursive").await?;
        let has_profile_id = self.has_column("watch_dirs", "profile_id").await?;
        let preserved_profiles = if has_profile_id {
            let rows = sqlx::query("SELECT path, profile_id FROM watch_dirs")
                .fetch_all(&self.pool)
                .await?;
            rows.into_iter()
                .map(|row| {
                    let path: String = row.get("path");
                    let profile_id: Option<i64> = row.get("profile_id");
                    (path, profile_id)
                })
                .collect::<HashMap<_, _>>()
        } else {
            HashMap::new()
        };
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM watch_dirs")
            .execute(&mut *tx)
            .await?;
        for watch_dir in watch_dirs {
            let preserved_profile_id = preserved_profiles.get(&watch_dir.path).copied().flatten();
            if has_is_recursive && has_profile_id {
                sqlx::query(
                    "INSERT INTO watch_dirs (path, is_recursive, profile_id) VALUES (?, ?, ?)",
                )
                .bind(&watch_dir.path)
                .bind(watch_dir.is_recursive)
                .bind(preserved_profile_id)
                .execute(&mut *tx)
                .await?;
            } else if has_recursive && has_profile_id {
                sqlx::query(
                    "INSERT INTO watch_dirs (path, recursive, profile_id) VALUES (?, ?, ?)",
                )
                .bind(&watch_dir.path)
                .bind(watch_dir.is_recursive)
                .bind(preserved_profile_id)
                .execute(&mut *tx)
                .await?;
            } else if has_recursive {
                sqlx::query("INSERT INTO watch_dirs (path, recursive) VALUES (?, ?)")
                    .bind(&watch_dir.path)
                    .bind(watch_dir.is_recursive)
                    .execute(&mut *tx)
                    .await?;
            } else {
                sqlx::query("INSERT INTO watch_dirs (path) VALUES (?)")
                    .bind(&watch_dir.path)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn remove_watch_dir(&self, id: i64) -> Result<()> {
        let res = sqlx::query("DELETE FROM watch_dirs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if res.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }
        Ok(())
    }

    pub async fn get_all_profiles(&self) -> Result<Vec<LibraryProfile>> {
        let profiles = sqlx::query_as::<_, LibraryProfile>(
            "SELECT id, name, preset, codec, quality_profile, hdr_mode, audio_mode,
                    crf_override, notes, created_at, updated_at
             FROM library_profiles
             ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(profiles)
    }

    pub async fn get_profile(&self, id: i64) -> Result<Option<LibraryProfile>> {
        let profile = sqlx::query_as::<_, LibraryProfile>(
            "SELECT id, name, preset, codec, quality_profile, hdr_mode, audio_mode,
                    crf_override, notes, created_at, updated_at
             FROM library_profiles
             WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(profile)
    }

    pub async fn create_profile(&self, profile: NewLibraryProfile) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO library_profiles
                (name, preset, codec, quality_profile, hdr_mode, audio_mode, crf_override, notes, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
        )
        .bind(profile.name)
        .bind(profile.preset)
        .bind(profile.codec)
        .bind(profile.quality_profile)
        .bind(profile.hdr_mode)
        .bind(profile.audio_mode)
        .bind(profile.crf_override)
        .bind(profile.notes)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    pub async fn update_profile(&self, id: i64, profile: NewLibraryProfile) -> Result<()> {
        let result = sqlx::query(
            "UPDATE library_profiles
             SET name = ?,
                 preset = ?,
                 codec = ?,
                 quality_profile = ?,
                 hdr_mode = ?,
                 audio_mode = ?,
                 crf_override = ?,
                 notes = ?,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
        )
        .bind(profile.name)
        .bind(profile.preset)
        .bind(profile.codec)
        .bind(profile.quality_profile)
        .bind(profile.hdr_mode)
        .bind(profile.audio_mode)
        .bind(profile.crf_override)
        .bind(profile.notes)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }

        Ok(())
    }

    pub async fn delete_profile(&self, id: i64) -> Result<()> {
        let result = sqlx::query("DELETE FROM library_profiles WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }
        Ok(())
    }

    pub async fn assign_profile_to_watch_dir(
        &self,
        dir_id: i64,
        profile_id: Option<i64>,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE watch_dirs
             SET profile_id = ?
             WHERE id = ?",
        )
        .bind(profile_id)
        .bind(dir_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }

        Ok(())
    }

    pub async fn get_profile_for_path(&self, path: &str) -> Result<Option<LibraryProfile>> {
        let normalized = Path::new(path);
        let candidate = sqlx::query_as::<_, LibraryProfile>(
            "SELECT lp.id, lp.name, lp.preset, lp.codec, lp.quality_profile, lp.hdr_mode,
                    lp.audio_mode, lp.crf_override, lp.notes, lp.created_at, lp.updated_at
             FROM watch_dirs wd
             JOIN library_profiles lp ON lp.id = wd.profile_id
             WHERE wd.profile_id IS NOT NULL
               AND (? = wd.path OR ? LIKE wd.path || '/%' OR ? LIKE wd.path || '\\%')
             ORDER BY LENGTH(wd.path) DESC
             LIMIT 1",
        )
        .bind(path)
        .bind(path)
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;

        if candidate.is_some() {
            return Ok(candidate);
        }

        // SQLite prefix matching is a fast first pass; fall back to strict path ancestry
        // if separators or normalization differ.
        let rows = sqlx::query(
            "SELECT wd.path,
                    lp.id, lp.name, lp.preset, lp.codec, lp.quality_profile, lp.hdr_mode,
                    lp.audio_mode, lp.crf_override, lp.notes, lp.created_at, lp.updated_at
             FROM watch_dirs wd
             JOIN library_profiles lp ON lp.id = wd.profile_id
             WHERE wd.profile_id IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;

        let mut best: Option<(usize, LibraryProfile)> = None;
        for row in rows {
            let watch_path: String = row.get("path");
            let profile = LibraryProfile {
                id: row.get("id"),
                name: row.get("name"),
                preset: row.get("preset"),
                codec: row.get("codec"),
                quality_profile: row.get("quality_profile"),
                hdr_mode: row.get("hdr_mode"),
                audio_mode: row.get("audio_mode"),
                crf_override: row.get("crf_override"),
                notes: row.get("notes"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            };
            let watch_path_buf = PathBuf::from(&watch_path);
            if normalized == watch_path_buf || normalized.starts_with(&watch_path_buf) {
                let score = watch_path.len();
                if best
                    .as_ref()
                    .is_none_or(|(best_score, _)| score > *best_score)
                {
                    best = Some((score, profile));
                }
            }
        }

        Ok(best.map(|(_, profile)| profile))
    }

    pub async fn count_watch_dirs_using_profile(&self, profile_id: i64) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM watch_dirs WHERE profile_id = ?")
            .bind(profile_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    pub async fn get_notification_targets(&self) -> Result<Vec<NotificationTarget>> {
        let targets = sqlx::query_as::<_, NotificationTarget>("SELECT id, name, target_type, endpoint_url, auth_token, events, enabled, created_at FROM notification_targets")
            .fetch_all(&self.pool)
            .await?;
        Ok(targets)
    }

    pub async fn add_notification_target(
        &self,
        name: &str,
        target_type: &str,
        endpoint_url: &str,
        auth_token: Option<&str>,
        events: &str,
        enabled: bool,
    ) -> Result<NotificationTarget> {
        let row = sqlx::query_as::<_, NotificationTarget>(
            "INSERT INTO notification_targets (name, target_type, endpoint_url, auth_token, events, enabled) 
             VALUES (?, ?, ?, ?, ?, ?) RETURNING *"
        )
        .bind(name)
        .bind(target_type)
        .bind(endpoint_url)
        .bind(auth_token)
        .bind(events)
        .bind(enabled)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete_notification_target(&self, id: i64) -> Result<()> {
        let res = sqlx::query("DELETE FROM notification_targets WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if res.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }
        Ok(())
    }

    pub async fn replace_notification_targets(
        &self,
        targets: &[crate::config::NotificationTargetConfig],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM notification_targets")
            .execute(&mut *tx)
            .await?;
        for target in targets {
            sqlx::query(
                "INSERT INTO notification_targets (name, target_type, endpoint_url, auth_token, events, enabled) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&target.name)
            .bind(&target.target_type)
            .bind(&target.endpoint_url)
            .bind(target.auth_token.as_deref())
            .bind(serde_json::to_string(&target.events).unwrap_or_else(|_| "[]".to_string()))
            .bind(target.enabled)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_schedule_windows(&self) -> Result<Vec<ScheduleWindow>> {
        let windows = sqlx::query_as::<_, ScheduleWindow>("SELECT * FROM schedule_windows")
            .fetch_all(&self.pool)
            .await?;
        Ok(windows)
    }

    pub async fn add_schedule_window(
        &self,
        start_time: &str,
        end_time: &str,
        days_of_week: &str,
        enabled: bool,
    ) -> Result<ScheduleWindow> {
        let row = sqlx::query_as::<_, ScheduleWindow>(
            "INSERT INTO schedule_windows (start_time, end_time, days_of_week, enabled) 
            VALUES (?, ?, ?, ?) 
            RETURNING *",
        )
        .bind(start_time)
        .bind(end_time)
        .bind(days_of_week)
        .bind(enabled)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete_schedule_window(&self, id: i64) -> Result<()> {
        let res = sqlx::query("DELETE FROM schedule_windows WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if res.rows_affected() == 0 {
            return Err(crate::error::AlchemistError::Database(
                sqlx::Error::RowNotFound,
            ));
        }
        Ok(())
    }

    pub async fn replace_schedule_windows(
        &self,
        windows: &[crate::config::ScheduleWindowConfig],
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM schedule_windows")
            .execute(&mut *tx)
            .await?;
        for window in windows {
            sqlx::query(
                "INSERT INTO schedule_windows (start_time, end_time, days_of_week, enabled) VALUES (?, ?, ?, ?)",
            )
            .bind(&window.start_time)
            .bind(&window.end_time)
            .bind(serde_json::to_string(&window.days_of_week).unwrap_or_else(|_| "[]".to_string()))
            .bind(window.enabled)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_file_settings(&self) -> Result<FileSettings> {
        // Migration ensures row 1 exists, but we handle missing just in case
        let row = sqlx::query_as::<_, FileSettings>("SELECT * FROM file_settings WHERE id = 1")
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(s) => Ok(s),
            None => {
                // If missing (shouldn't happen), return default
                Ok(FileSettings {
                    id: 1,
                    delete_source: false,
                    output_extension: "mkv".to_string(),
                    output_suffix: "-alchemist".to_string(),
                    replace_strategy: "keep".to_string(),
                    output_root: None,
                })
            }
        }
    }

    pub async fn update_file_settings(
        &self,
        delete_source: bool,
        output_extension: &str,
        output_suffix: &str,
        replace_strategy: &str,
        output_root: Option<&str>,
    ) -> Result<FileSettings> {
        let row = sqlx::query_as::<_, FileSettings>(
            "UPDATE file_settings 
            SET delete_source = ?, output_extension = ?, output_suffix = ?, replace_strategy = ?, output_root = ?
            WHERE id = 1
            RETURNING *",
        )
        .bind(delete_source)
        .bind(output_extension)
        .bind(output_suffix)
        .bind(replace_strategy)
        .bind(output_root)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn replace_file_settings_projection(
        &self,
        settings: &crate::config::FileSettingsConfig,
    ) -> Result<FileSettings> {
        self.update_file_settings(
            settings.delete_source,
            &settings.output_extension,
            &settings.output_suffix,
            &settings.replace_strategy,
            settings.output_root.as_deref(),
        )
        .await
    }

    pub async fn get_aggregated_stats(&self) -> Result<AggregatedStats> {
        let pool = &self.pool;
        timed_query("get_aggregated_stats", || async {
            let row = sqlx::query(
                "SELECT 
                    (SELECT COUNT(*) FROM jobs) as total_jobs,
                    (SELECT COUNT(*) FROM jobs WHERE status = 'completed') as completed_jobs,
                    COALESCE(SUM(input_size_bytes), 0) as total_input_size,
                    COALESCE(SUM(output_size_bytes), 0) as total_output_size,
                    AVG(vmaf_score) as avg_vmaf,
                    COALESCE(SUM(encode_time_seconds), 0.0) as total_encode_time
                 FROM encode_stats",
            )
            .fetch_one(pool)
            .await?;

            Ok(AggregatedStats {
                total_jobs: row.get("total_jobs"),
                completed_jobs: row.get("completed_jobs"),
                total_input_size: row.get("total_input_size"),
                total_output_size: row.get("total_output_size"),
                avg_vmaf: row.get("avg_vmaf"),
                total_encode_time_seconds: row.get("total_encode_time"),
            })
        })
        .await
    }

    /// Get daily statistics for the last N days (for time-series charts)
    pub async fn get_daily_stats(&self, days: i32) -> Result<Vec<DailyStats>> {
        let pool = &self.pool;
        let days_str = format!("-{}", days);
        timed_query("get_daily_stats", || async {
            let rows = sqlx::query(
                "SELECT 
                    DATE(e.created_at) as date,
                    COUNT(*) as jobs_completed,
                    COALESCE(SUM(e.input_size_bytes - e.output_size_bytes), 0) as bytes_saved,
                    COALESCE(SUM(e.input_size_bytes), 0) as total_input_bytes,
                    COALESCE(SUM(e.output_size_bytes), 0) as total_output_bytes
                 FROM encode_stats e
                 WHERE e.created_at >= DATE('now', ? || ' days')
                 GROUP BY DATE(e.created_at)
                 ORDER BY date ASC",
            )
            .bind(&days_str)
            .fetch_all(pool)
            .await?;

            let stats = rows
                .iter()
                .map(|row| DailyStats {
                    date: row.get("date"),
                    jobs_completed: row.get("jobs_completed"),
                    bytes_saved: row.get("bytes_saved"),
                    total_input_bytes: row.get("total_input_bytes"),
                    total_output_bytes: row.get("total_output_bytes"),
                })
                .collect();

            Ok(stats)
        })
        .await
    }

    /// Get detailed per-job encoding statistics (most recent first)
    pub async fn get_detailed_encode_stats(&self, limit: i32) -> Result<Vec<DetailedEncodeStats>> {
        let pool = &self.pool;
        timed_query("get_detailed_encode_stats", || async {
            let stats = sqlx::query_as::<_, DetailedEncodeStats>(
                "SELECT 
                    e.job_id,
                    j.input_path,
                    e.input_size_bytes,
                    e.output_size_bytes,
                    e.compression_ratio,
                    e.encode_time_seconds,
                    e.encode_speed,
                    e.avg_bitrate_kbps,
                    e.vmaf_score,
                    e.created_at
                 FROM encode_stats e
                 JOIN jobs j ON e.job_id = j.id
                 ORDER BY e.created_at DESC
                 LIMIT ?",
            )
            .bind(limit)
            .fetch_all(pool)
            .await?;

            Ok(stats)
        })
        .await
    }

    pub async fn get_savings_summary(&self) -> Result<SavingsSummary> {
        let pool = &self.pool;
        timed_query("get_savings_summary", || async {
            let totals = sqlx::query(
                "SELECT
                    COALESCE(SUM(input_size_bytes), 0) as total_input_bytes,
                    COALESCE(SUM(output_size_bytes), 0) as total_output_bytes,
                    COUNT(*) as job_count
                 FROM encode_stats
                 WHERE output_size_bytes IS NOT NULL",
            )
            .fetch_one(pool)
            .await?;

            let total_input_bytes: i64 = totals.get("total_input_bytes");
            let total_output_bytes: i64 = totals.get("total_output_bytes");
            let job_count: i64 = totals.get("job_count");
            let total_bytes_saved = (total_input_bytes - total_output_bytes).max(0);
            let savings_percent = if total_input_bytes > 0 {
                (total_bytes_saved as f64 / total_input_bytes as f64) * 100.0
            } else {
                0.0
            };

            let savings_by_codec = sqlx::query(
                "SELECT
                    COALESCE(NULLIF(TRIM(e.output_codec), ''), 'unknown') as codec,
                    COALESCE(SUM(e.input_size_bytes - e.output_size_bytes), 0) as bytes_saved
                 FROM encode_stats e
                 JOIN jobs j ON j.id = e.job_id
                 WHERE e.output_size_bytes IS NOT NULL
                 GROUP BY codec
                 ORDER BY bytes_saved DESC, codec ASC",
            )
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| CodecSavings {
                codec: row.get("codec"),
                bytes_saved: row.get("bytes_saved"),
            })
            .collect::<Vec<_>>();

            let savings_over_time = sqlx::query(
                "SELECT
                    DATE(e.created_at) as date,
                    COALESCE(SUM(e.input_size_bytes - e.output_size_bytes), 0) as bytes_saved
                 FROM encode_stats e
                 WHERE e.output_size_bytes IS NOT NULL
                   AND e.created_at >= datetime('now', '-30 days')
                 GROUP BY DATE(e.created_at)
                 ORDER BY date ASC",
            )
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|row| DailySavings {
                date: row.get("date"),
                bytes_saved: row.get("bytes_saved"),
            })
            .collect::<Vec<_>>();

            Ok(SavingsSummary {
                total_input_bytes,
                total_output_bytes,
                total_bytes_saved,
                savings_percent,
                job_count,
                savings_by_codec,
                savings_over_time,
            })
        })
        .await
    }

    /// Batch update job statuses (for batch operations)
    pub async fn batch_update_status(
        &self,
        status_from: JobState,
        status_to: JobState,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE jobs SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE status = ?",
        )
        .bind(status_to)
        .bind(status_from)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Increment attempt count
    pub async fn increment_attempt_count(&self, id: i64) -> Result<()> {
        sqlx::query("UPDATE jobs SET attempt_count = attempt_count + 1 WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn restart_failed_jobs(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE jobs
             SET status = 'queued', progress = 0.0, updated_at = CURRENT_TIMESTAMP
             WHERE status IN ('failed', 'cancelled') AND archived = 0",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn clear_completed_jobs(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE jobs
             SET archived = 1, updated_at = CURRENT_TIMESTAMP
             WHERE status = 'completed' AND archived = 0",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Set UI preference
    pub async fn set_preference(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO ui_preferences (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get UI preference
    pub async fn get_preference(&self, key: &str) -> Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM ui_preferences WHERE key = ?")
                .bind(key)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    pub async fn delete_preference(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM ui_preferences WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_job_stats(&self) -> Result<JobStats> {
        let pool = &self.pool;
        timed_query("get_job_stats", || async {
            let rows = sqlx::query("SELECT status, COUNT(*) as count FROM jobs GROUP BY status")
                .fetch_all(pool)
                .await?;

            let mut stats = JobStats::default();
            for row in rows {
                let status_str: String = row.get("status");
                let count: i64 = row.get("count");

                // Map status string to JobStats fields
                // Assuming JobState serialization matches stored strings ("queued", "active", etc)
                match status_str.as_str() {
                    "queued" => stats.queued += count,
                    "encoding" | "analyzing" | "remuxing" | "resuming" => stats.active += count,
                    "completed" => stats.completed += count,
                    "failed" | "cancelled" => stats.failed += count,
                    _ => {}
                }
            }
            Ok(stats)
        })
        .await
    }

    pub async fn add_log(&self, level: &str, job_id: Option<i64>, message: &str) -> Result<()> {
        sqlx::query("INSERT INTO logs (level, job_id, message) VALUES (?, ?, ?)")
            .bind(level)
            .bind(job_id)
            .bind(message)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_logs(&self, limit: i64, offset: i64) -> Result<Vec<LogEntry>> {
        let logs = sqlx::query_as::<_, LogEntry>(
            "SELECT id, level, job_id, message, created_at FROM logs ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(logs)
    }

    pub async fn get_logs_for_job(&self, job_id: i64, limit: i64) -> Result<Vec<LogEntry>> {
        sqlx::query_as::<_, LogEntry>(
            "SELECT id, level, job_id, message, created_at
             FROM logs
             WHERE job_id = ?
             ORDER BY created_at ASC
             LIMIT ?",
        )
        .bind(job_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn clear_logs(&self) -> Result<()> {
        sqlx::query("DELETE FROM logs").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn prune_old_logs(&self, max_age_days: u32) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM logs
             WHERE created_at < datetime('now', '-' || ? || ' days')",
        )
        .bind(max_age_days as i64)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn create_user(&self, username: &str, password_hash: &str) -> Result<i64> {
        let id = sqlx::query("INSERT INTO users (username, password_hash) VALUES (?, ?)")
            .bind(username)
            .bind(password_hash)
            .execute(&self.pool)
            .await?
            .last_insert_rowid();
        Ok(id)
    }

    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    pub async fn has_users(&self) -> Result<bool> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0 > 0)
    }

    pub async fn create_session(
        &self,
        user_id: i64,
        token: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<()> {
        let token_hash = hash_session_token(token);
        sqlx::query("INSERT INTO sessions (token, user_id, expires_at) VALUES (?, ?, ?)")
            .bind(token_hash)
            .bind(user_id)
            .bind(expires_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_session(&self, token: &str) -> Result<Option<Session>> {
        let token_hash = hash_session_token(token);
        if let Some(session) = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE token = ? AND expires_at > CURRENT_TIMESTAMP",
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(Some(session));
        }

        let mut session = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE token = ? AND expires_at > CURRENT_TIMESTAMP",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(existing) = session.as_mut() {
            let _ = sqlx::query("UPDATE sessions SET token = ? WHERE token = ?")
                .bind(&token_hash)
                .bind(token)
                .execute(&self.pool)
                .await;
            existing.token = token_hash;
        }

        Ok(session)
    }

    pub async fn delete_session(&self, token: &str) -> Result<()> {
        let token_hash = hash_session_token(token);
        sqlx::query("DELETE FROM sessions WHERE token = ? OR token = ?")
            .bind(&token_hash)
            .bind(token)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn cleanup_sessions(&self) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE expires_at <= CURRENT_TIMESTAMP")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn cleanup_expired_sessions(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= CURRENT_TIMESTAMP")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn record_health_check(
        &self,
        job_id: i64,
        issues: Option<&crate::media::health::HealthIssueReport>,
    ) -> Result<()> {
        let serialized_issues = issues
            .map(serde_json::to_string)
            .transpose()
            .map_err(|err| {
                crate::error::AlchemistError::Unknown(format!(
                    "Failed to serialize health issue report: {}",
                    err
                ))
            })?;

        sqlx::query(
            "UPDATE jobs
             SET health_issues = ?,
                 last_health_check = datetime('now')
             WHERE id = ?",
        )
        .bind(serialized_issues)
        .bind(job_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_health_summary(&self) -> Result<HealthSummary> {
        let pool = &self.pool;
        timed_query("get_health_summary", || async {
            let row = sqlx::query(
                "SELECT
                    (SELECT COUNT(*) FROM jobs WHERE last_health_check IS NOT NULL) as total_checked,
                    (SELECT COUNT(*)
                     FROM jobs
                     WHERE health_issues IS NOT NULL AND TRIM(health_issues) != '') as issues_found,
                    (SELECT MAX(started_at) FROM health_scan_runs) as last_run",
            )
            .fetch_one(pool)
            .await?;

            Ok(HealthSummary {
                total_checked: row.get("total_checked"),
                issues_found: row.get("issues_found"),
                last_run: row.get("last_run"),
            })
        })
        .await
    }

    pub async fn create_health_scan_run(&self) -> Result<i64> {
        let id = sqlx::query("INSERT INTO health_scan_runs DEFAULT VALUES")
            .execute(&self.pool)
            .await?
            .last_insert_rowid();
        Ok(id)
    }

    pub async fn complete_health_scan_run(
        &self,
        id: i64,
        files_checked: i64,
        issues_found: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE health_scan_runs
             SET completed_at = datetime('now'),
                 files_checked = ?,
                 issues_found = ?
             WHERE id = ?",
        )
        .bind(files_checked)
        .bind(issues_found)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_jobs_needing_health_check(&self) -> Result<Vec<Job>> {
        let pool = &self.pool;
        timed_query("get_jobs_needing_health_check", || async {
            let jobs = sqlx::query_as::<_, Job>(
                "SELECT j.id, j.input_path, j.output_path, j.status,
                        (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                        COALESCE(j.priority, 0) as priority,
                        COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                        COALESCE(j.attempt_count, 0) as attempt_count,
                        (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                        j.created_at, j.updated_at
                 FROM jobs j
                 WHERE j.status = 'completed'
                   AND (
                        j.last_health_check IS NULL
                        OR j.last_health_check < datetime('now', '-7 days')
                   )
                 ORDER BY COALESCE(j.last_health_check, '1970-01-01') ASC, j.updated_at DESC",
            )
            .fetch_all(pool)
            .await?;
            Ok(jobs)
        })
        .await
    }

    pub async fn get_jobs_with_health_issues(&self) -> Result<Vec<JobWithHealthIssueRow>> {
        let pool = &self.pool;
        timed_query("get_jobs_with_health_issues", || async {
            let jobs = sqlx::query_as::<_, JobWithHealthIssueRow>(
                "SELECT j.id, j.input_path, j.output_path, j.status,
                        (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                        COALESCE(j.priority, 0) as priority,
                        COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                        COALESCE(j.attempt_count, 0) as attempt_count,
                        (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                        j.created_at, j.updated_at,
                        j.health_issues
                 FROM jobs j
                 WHERE j.archived = 0
                   AND j.health_issues IS NOT NULL
                   AND TRIM(j.health_issues) != ''
                 ORDER BY j.updated_at DESC",
            )
            .fetch_all(pool)
            .await?;
            Ok(jobs)
        })
        .await
    }

    pub async fn reset_auth(&self) -> Result<()> {
        sqlx::query("DELETE FROM sessions")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM users").execute(&self.pool).await?;
        Ok(())
    }

    async fn has_column(&self, table: &str, column: &str) -> Result<bool> {
        let sql = "SELECT name FROM pragma_table_info(?) WHERE name = ?";
        let row = sqlx::query(sql)
            .bind(table)
            .bind(column)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.is_some())
    }
}

// Auth related structs
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Session {
    pub token: String,
    pub user_id: i64,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Hash a session token using SHA256 for secure storage.
///
/// # Security: Timing Attack Resistance
///
/// Session tokens are hashed before storage and lookup. Token validation uses
/// SQL `WHERE token = ?` with the hashed value, so the comparison occurs in
/// SQLite rather than in Rust code. This is inherently constant-time from the
/// application's perspective because:
/// 1. The database performs the comparison, not our code
/// 2. Database query time doesn't leak information about partial matches
/// 3. No early-exit comparison in application code
///
/// This design makes timing attacks infeasible without requiring the `subtle`
/// crate for constant-time comparison.
fn hash_session_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{:02x}", byte);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    #[test]
    fn test_output_path_for_suffix() {
        let settings = FileSettings {
            id: 1,
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "-alchemist".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: None,
        };
        let input = Path::new("video.mp4");
        let output = settings.output_path_for(input);
        assert_eq!(output, PathBuf::from("video-alchemist.mkv"));
    }

    #[test]
    fn test_output_path_avoids_inplace() {
        let settings = FileSettings {
            id: 1,
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: None,
        };
        let input = Path::new("video.mkv");
        let output = settings.output_path_for(input);
        assert_ne!(output, input);
    }

    #[test]
    fn test_output_path_mirrors_source_root_under_output_root() {
        let settings = FileSettings {
            id: 1,
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "-alchemist".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: Some("/encoded".to_string()),
        };
        let input = Path::new("/library/movies/action/video.mp4");
        let output = settings.output_path_for_source(input, Some(Path::new("/library")));
        assert_eq!(
            output,
            PathBuf::from("/encoded/movies/action/video-alchemist.mkv")
        );
    }

    #[test]
    fn test_output_path_falls_back_when_source_root_does_not_match() {
        let settings = FileSettings {
            id: 1,
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "-alchemist".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: Some("/encoded".to_string()),
        };
        let input = Path::new("/library/movies/video.mp4");
        let output = settings.output_path_for_source(input, Some(Path::new("/other")));
        assert_eq!(output, PathBuf::from("/library/movies/video-alchemist.mkv"));
    }

    #[test]
    fn test_replace_strategy() {
        let mut settings = FileSettings {
            id: 1,
            delete_source: false,
            output_extension: "mkv".to_string(),
            output_suffix: "-alchemist".to_string(),
            replace_strategy: "keep".to_string(),
            output_root: None,
        };
        assert!(!settings.should_replace_existing_output());
        settings.replace_strategy = "replace".to_string();
        assert!(settings.should_replace_existing_output());
    }

    #[tokio::test]
    async fn test_enqueue_job_reports_change_state()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_enqueue_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        let input = Path::new("input.mkv");
        let output = Path::new("output.mkv");
        let changed = db
            .enqueue_job(input, output, SystemTime::UNIX_EPOCH)
            .await?;
        assert!(changed);

        let unchanged = db
            .enqueue_job(input, output, SystemTime::UNIX_EPOCH)
            .await?;
        assert!(!unchanged);

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn test_claim_next_job_marks_analyzing()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        let input1 = Path::new("input1.mkv");
        let output1 = Path::new("output1.mkv");
        let _ = db
            .enqueue_job(input1, output1, SystemTime::UNIX_EPOCH)
            .await?;

        let input2 = Path::new("input2.mkv");
        let output2 = Path::new("output2.mkv");
        let _ = db
            .enqueue_job(input2, output2, SystemTime::UNIX_EPOCH)
            .await?;

        let first = db
            .claim_next_job()
            .await?
            .ok_or_else(|| std::io::Error::other("missing job 1"))?;
        assert_eq!(first.status, JobState::Analyzing);

        let second = db
            .claim_next_job()
            .await?
            .ok_or_else(|| std::io::Error::other("missing job 2"))?;
        assert_ne!(first.id, second.id);

        let none = db.claim_next_job().await?;
        assert!(none.is_none());

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn claim_next_job_respects_attempt_backoff()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_backoff_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let input = Path::new("backoff-input.mkv");
        let output = Path::new("backoff-output.mkv");
        let _ = db
            .enqueue_job(input, output, SystemTime::UNIX_EPOCH)
            .await?;

        let job = db
            .get_job_by_input_path("backoff-input.mkv")
            .await?
            .ok_or_else(|| std::io::Error::other("missing backoff job"))?;

        sqlx::query(
            "UPDATE jobs
             SET attempt_count = 1,
                 updated_at = datetime('now')
             WHERE id = ?",
        )
        .bind(job.id)
        .execute(&db.pool)
        .await?;

        assert!(db.claim_next_job().await?.is_none());

        sqlx::query(
            "UPDATE jobs
             SET updated_at = datetime('now', '-6 minutes')
             WHERE id = ?",
        )
        .bind(job.id)
        .execute(&db.pool)
        .await?;

        let claimed = db.claim_next_job().await?;
        assert!(claimed.is_some());

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn clear_completed_archives_jobs_but_preserves_encode_stats()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_archive_completed_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let input = Path::new("movie.mkv");
        let output = Path::new("movie-alchemist.mkv");
        let _ = db
            .enqueue_job(input, output, SystemTime::UNIX_EPOCH)
            .await?;

        let job = db
            .get_job_by_input_path("movie.mkv")
            .await?
            .ok_or_else(|| std::io::Error::other("missing job"))?;
        db.update_job_status(job.id, JobState::Completed).await?;
        db.save_encode_stats(EncodeStatsInput {
            job_id: job.id,
            input_size: 2_000,
            output_size: 1_000,
            compression_ratio: 0.5,
            encode_time: 42.0,
            encode_speed: 1.2,
            avg_bitrate: 800.0,
            vmaf_score: Some(96.5),
            output_codec: Some("av1".to_string()),
        })
        .await?;

        let cleared = db.clear_completed_jobs().await?;
        assert_eq!(cleared, 1);
        assert!(db.get_job_by_id(job.id).await?.is_none());
        assert!(db.get_job_by_input_path("movie.mkv").await?.is_none());

        let visible_completed = db.get_jobs_by_status(JobState::Completed).await?;
        assert!(visible_completed.is_empty());

        let aggregated = db.get_aggregated_stats().await?;
        assert_eq!(aggregated.completed_jobs, 1);
        assert_eq!(aggregated.total_input_size, 2_000);
        assert_eq!(aggregated.total_output_size, 1_000);

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
