use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ssr", derive(sqlx::Type))]
#[cfg_attr(feature = "ssr", sqlx(rename_all = "lowercase"))]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Job {
    pub id: i64,
    pub input_path: String,
    pub output_path: String,
    pub status: JobState,
    pub decision_reason: Option<String>,
    pub priority: i32,
    pub progress: f64,
    pub attempt_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Decision {
    pub id: i64,
    pub job_id: i64,
    pub action: String, // "encode", "skip", "reject"
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

#[cfg(feature = "ssr")]
pub struct Db {
    pool: SqlitePool,
}

#[cfg(feature = "ssr")]
impl Db {
    pub async fn new(db_path: &str) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(options).await?;

        let db = Self { pool };
        db.init().await?;
        db.reset_interrupted_jobs().await?;

        Ok(db)
    }

    async fn init(&self) -> Result<()> {
        // Jobs table with priority and progress
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                input_path TEXT NOT NULL UNIQUE,
                output_path TEXT NOT NULL,
                status TEXT NOT NULL,
                mtime_hash TEXT NOT NULL,
                priority INTEGER DEFAULT 0,
                progress REAL DEFAULT 0.0,
                attempt_count INTEGER DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(&self.pool)
        .await?;

        // Add columns if they don't exist (migrations for existing DBs)
        let _ = sqlx::query("ALTER TABLE jobs ADD COLUMN priority INTEGER DEFAULT 0")
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("ALTER TABLE jobs ADD COLUMN progress REAL DEFAULT 0.0")
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("ALTER TABLE jobs ADD COLUMN attempt_count INTEGER DEFAULT 0")
            .execute(&self.pool)
            .await;

        // Decisions table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS decisions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id INTEGER NOT NULL,
                action TEXT NOT NULL,
                reason TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(job_id) REFERENCES jobs(id)
            )",
        )
        .execute(&self.pool)
        .await?;

        // Encode stats table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS encode_stats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id INTEGER NOT NULL UNIQUE,
                input_size_bytes INTEGER NOT NULL,
                output_size_bytes INTEGER NOT NULL,
                compression_ratio REAL NOT NULL,
                encode_time_seconds REAL NOT NULL,
                encode_speed REAL NOT NULL,
                avg_bitrate_kbps REAL NOT NULL,
                vmaf_score REAL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(job_id) REFERENCES jobs(id)
            )",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

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

        // simple mtime hash
        let mtime_hash = format!("{:?}", mtime);

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
                    d.reason as decision_reason,
                    COALESCE(j.priority, 0) as priority, 
                    COALESCE(j.progress, 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    j.created_at, j.updated_at
             FROM jobs j
             LEFT JOIN decisions d ON j.id = d.job_id
             GROUP BY j.id
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
                    d.reason as decision_reason,
                    COALESCE(j.priority, 0) as priority, 
                    COALESCE(j.progress, 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    j.created_at, j.updated_at
             FROM jobs j
             LEFT JOIN decisions d ON j.id = d.job_id
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
                    d.reason as decision_reason,
                    COALESCE(j.priority, 0) as priority, 
                    COALESCE(j.progress, 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    j.created_at, j.updated_at
             FROM jobs j
             LEFT JOIN decisions d ON j.id = d.job_id
             WHERE j.status = ?
             GROUP BY j.id
             ORDER BY j.priority DESC, j.created_at ASC",
        )
        .bind(status)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
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
}
