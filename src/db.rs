use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteConnectOptions, Row, SqlitePool};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Analyzing,
    Encoding,
    Completed,
    Skipped,
    Failed,
    Cancelled,
    Resuming,
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
        job_id: i64,
        message: String,
    },
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            JobState::Queued => "queued",
            JobState::Analyzing => "analyzing",
            JobState::Encoding => "encoding",
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

impl Job {
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            JobState::Encoding | JobState::Analyzing | JobState::Resuming
        )
    }

    pub fn can_retry(&self) -> bool {
        matches!(self.status, JobState::Failed | JobState::Cancelled)
    }

    pub fn status_class(&self) -> &'static str {
        match self.status {
            JobState::Completed => "badge-green",
            JobState::Encoding | JobState::Resuming => "badge-yellow",
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

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Decision {
    pub id: i64,
    pub job_id: i64,
    pub action: String, // "encode", "skip", "reject"
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

pub struct Db {
    pool: SqlitePool,
}

impl Db {
    pub async fn new(db_path: &str) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(options).await?;

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| crate::error::AlchemistError::Database(e.into()))?;

        let db = Self { pool };
        // db.reset_interrupted_jobs().await?; // Optional: keep or remove based on preference, but good for safety

        Ok(db)
    }

    // init method removed as it is replaced by migrations

    pub async fn reset_interrupted_jobs(&self) -> Result<()> {
        sqlx::query("UPDATE jobs SET status = 'queued' WHERE status IN ('analyzing', 'encoding')")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn enqueue_job(
        &self,
        input_path: &Path,
        output_path: &Path,
        mtime: std::time::SystemTime,
    ) -> Result<()> {
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

        sqlx::query(
            "INSERT INTO jobs (input_path, output_path, status, mtime_hash, updated_at) 
             VALUES (?, ?, 'queued', ?, CURRENT_TIMESTAMP)
             ON CONFLICT(input_path) DO UPDATE SET
             status = CASE WHEN mtime_hash != excluded.mtime_hash THEN 'queued' ELSE status END,
             mtime_hash = excluded.mtime_hash,
             updated_at = CURRENT_TIMESTAMP
             WHERE mtime_hash != excluded.mtime_hash",
        )
        .bind(input_str)
        .bind(output_str)
        .bind(mtime_hash)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_next_job(&self) -> Result<Option<Job>> {
        let job = sqlx::query_as::<_, Job>(
            "SELECT id, input_path, output_path, status, NULL as decision_reason,
                    COALESCE(priority, 0) as priority, COALESCE(progress, 0.0) as progress,
                    COALESCE(attempt_count, 0) as attempt_count,
                    NULL as vmaf_score,
                    created_at, updated_at 
             FROM jobs WHERE status = 'queued' 
             ORDER BY priority DESC, created_at ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(job)
    }

    pub async fn update_job_status(&self, id: i64, status: JobState) -> Result<()> {
        sqlx::query("UPDATE jobs SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;

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
        let jobs = sqlx::query_as::<_, Job>(
            "SELECT j.id, j.input_path, j.output_path, j.status, 
                    (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                    COALESCE(j.priority, 0) as priority, 
                    COALESCE(j.progress, 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                    j.created_at, j.updated_at
             FROM jobs j
             ORDER BY j.updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    pub async fn get_job_decision(&self, job_id: i64) -> Result<Option<Decision>> {
        let decision = sqlx::query_as::<_, Decision>(
            "SELECT * FROM decisions WHERE job_id = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(decision)
    }

    pub async fn get_stats(&self) -> Result<serde_json::Value> {
        let stats = sqlx::query("SELECT status, count(*) as count FROM jobs GROUP BY status")
            .fetch_all(&self.pool)
            .await?;

        let mut map = serde_json::Map::new();
        for row in stats {
            use sqlx::Row;
            let status: String = row.get("status");
            let count: i64 = row.get("count");
            map.insert(status, serde_json::Value::Number(count.into()));
        }

        Ok(serde_json::Value::Object(map))
    }

    /// Update job progress (for resume support)
    pub async fn update_job_progress(&self, id: i64, progress: f64) -> Result<()> {
        sqlx::query("UPDATE jobs SET progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(progress)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Set job priority
    pub async fn set_job_priority(&self, id: i64, priority: i32) -> Result<()> {
        sqlx::query("UPDATE jobs SET priority = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(priority)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Save encode statistics
    pub async fn save_encode_stats(
        &self,
        job_id: i64,
        input_size: u64,
        output_size: u64,
        compression_ratio: f64,
        encode_time: f64,
        encode_speed: f64,
        avg_bitrate: f64,
        vmaf_score: Option<f64>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO encode_stats 
             (job_id, input_size_bytes, output_size_bytes, compression_ratio, 
              encode_time_seconds, encode_speed, avg_bitrate_kbps, vmaf_score)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(job_id) DO UPDATE SET
             input_size_bytes = excluded.input_size_bytes,
             output_size_bytes = excluded.output_size_bytes,
             compression_ratio = excluded.compression_ratio,
             encode_time_seconds = excluded.encode_time_seconds,
             encode_speed = excluded.encode_speed,
             avg_bitrate_kbps = excluded.avg_bitrate_kbps,
             vmaf_score = excluded.vmaf_score",
        )
        .bind(job_id)
        .bind(input_size as i64)
        .bind(output_size as i64)
        .bind(compression_ratio)
        .bind(encode_time)
        .bind(encode_speed)
        .bind(avg_bitrate)
        .bind(vmaf_score)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get job by ID
    pub async fn get_job(&self, id: i64) -> Result<Option<Job>> {
        let job = sqlx::query_as::<_, Job>(
            "SELECT j.id, j.input_path, j.output_path, j.status, 
                    (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                    COALESCE(j.priority, 0) as priority, 
                    COALESCE(j.progress, 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                    j.created_at, j.updated_at
             FROM jobs j
             WHERE j.id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(job)
    }

    /// Get jobs by status
    pub async fn get_jobs_by_status(&self, status: JobState) -> Result<Vec<Job>> {
        let jobs = sqlx::query_as::<_, Job>(
            "SELECT j.id, j.input_path, j.output_path, j.status, 
                    (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                    COALESCE(j.priority, 0) as priority, 
                    COALESCE(j.progress, 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                    j.created_at, j.updated_at
             FROM jobs j
             WHERE j.status = ?
             ORDER BY j.priority DESC, j.created_at ASC",
        )
        .bind(status)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    pub async fn get_aggregated_stats(&self) -> Result<AggregatedStats> {
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
        .fetch_one(&self.pool)
        .await?;

        Ok(AggregatedStats {
            total_jobs: row.get("total_jobs"),
            completed_jobs: row.get("completed_jobs"),
            total_input_size: row.get("total_input_size"),
            total_output_size: row.get("total_output_size"),
            avg_vmaf: row.get("avg_vmaf"),
            total_encode_time_seconds: row.get("total_encode_time"),
        })
    }

    /// Get daily statistics for the last N days (for time-series charts)
    pub async fn get_daily_stats(&self, days: i32) -> Result<Vec<DailyStats>> {
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
        .bind(format!("-{}", days))
        .fetch_all(&self.pool)
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
    }

    /// Get detailed per-job encoding statistics (most recent first)
    pub async fn get_detailed_encode_stats(&self, limit: i32) -> Result<Vec<DetailedEncodeStats>> {
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
        .fetch_all(&self.pool)
        .await?;

        Ok(stats)
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
        let result = sqlx::query("UPDATE jobs SET status = 'queued', updated_at = CURRENT_TIMESTAMP WHERE status IN ('failed', 'cancelled')")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn clear_completed_jobs(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM jobs WHERE status = 'completed'")
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
        sqlx::query("INSERT INTO sessions (token, user_id, expires_at) VALUES (?, ?, ?)")
            .bind(token)
            .bind(user_id)
            .bind(expires_at)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_session(&self, token: &str) -> Result<Option<Session>> {
        let session = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE token = ? AND expires_at > CURRENT_TIMESTAMP",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await?;
        Ok(session)
    }

    pub async fn cleanup_sessions(&self) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE expires_at <= CURRENT_TIMESTAMP")
            .execute(&self.pool)
            .await?;
        Ok(())
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
