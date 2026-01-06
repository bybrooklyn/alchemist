use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::path::Path;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, PartialEq, Eq)]
#[sqlx(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Analyzing,
    Encoding,
    Completed,
    Skipped,
    Failed,
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
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct Job {
    pub id: i64,
    pub input_path: String,
    pub output_path: String,
    pub status: JobState,
    pub decision_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
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

        let db = Self { pool };
        db.init().await?;
        db.reset_interrupted_jobs().await?;
        
        Ok(db)
    }

    async fn init(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                input_path TEXT NOT NULL UNIQUE,
                output_path TEXT NOT NULL,
                status TEXT NOT NULL,
                mtime_hash TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS decisions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                job_id INTEGER NOT NULL,
                action TEXT NOT NULL,
                reason TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(job_id) REFERENCES jobs(id)
            )"
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn reset_interrupted_jobs(&self) -> Result<()> {
        sqlx::query(
            "UPDATE jobs SET status = 'queued' WHERE status IN ('analyzing', 'encoding')"
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn enqueue_job(&self, input_path: &Path, output_path: &Path, mtime: std::time::SystemTime) -> Result<()> {
        let input_str = input_path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid input path"))?;
        let output_str = output_path.to_str().ok_or_else(|| anyhow::anyhow!("Invalid output path"))?;
        
        // simple mtime hash
        let mtime_hash = format!("{:?}", mtime);

        sqlx::query(
            "INSERT INTO jobs (input_path, output_path, status, mtime_hash, updated_at) 
             VALUES (?, ?, 'queued', ?, CURRENT_TIMESTAMP)
             ON CONFLICT(input_path) DO UPDATE SET
             status = CASE WHEN mtime_hash != excluded.mtime_hash THEN 'queued' ELSE status END,
             mtime_hash = excluded.mtime_hash,
             updated_at = CURRENT_TIMESTAMP
             WHERE mtime_hash != excluded.mtime_hash"
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
            "SELECT id, input_path, output_path, status, created_at, updated_at 
             FROM jobs WHERE status = 'queued' ORDER BY created_at LIMIT 1"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(job)
    }

    pub async fn update_job_status(&self, id: i64, status: JobState) -> Result<()> {
        sqlx::query(
            "UPDATE jobs SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
        .bind(status)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn add_decision(&self, job_id: i64, action: &str, reason: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO decisions (job_id, action, reason) VALUES (?, ?, ?)"
        )
        .bind(job_id)
        .bind(action)
        .bind(reason)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_all_jobs(&self) -> Result<Vec<Job>> {
        let jobs = sqlx::query_as::<_, Job>(
            "SELECT j.id, j.input_path, j.output_path, j.status, j.created_at, j.updated_at, d.reason as decision_reason
             FROM jobs j
             LEFT JOIN decisions d ON j.id = d.job_id
             GROUP BY j.id
             ORDER BY j.updated_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    pub async fn get_job_decision(&self, job_id: i64) -> Result<Option<Decision>> {
        let decision = sqlx::query_as::<_, Decision>(
            "SELECT * FROM decisions WHERE job_id = ? ORDER BY created_at DESC LIMIT 1"
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(decision)
    }

    pub async fn get_stats(&self) -> Result<serde_json::Value> {
        let stats = sqlx::query(
            "SELECT status, count(*) as count FROM jobs GROUP BY status"
        )
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
}
