use chrono::{DateTime, Utc};
use crate::error::Result;
use sqlx::Row;

use super::timed_query;
use super::types::*;
use super::{hash_api_token, hash_session_token, Db};

impl Db {
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
        let session = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE token = ? AND expires_at > CURRENT_TIMESTAMP",
        )
        .bind(&token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(session)
    }

    pub async fn delete_session(&self, token: &str) -> Result<()> {
        let token_hash = hash_session_token(token);
        sqlx::query("DELETE FROM sessions WHERE token = ?")
            .bind(&token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_api_tokens(&self) -> Result<Vec<ApiToken>> {
        let tokens = sqlx::query_as::<_, ApiToken>(
            "SELECT id, name, access_level, created_at, last_used_at, revoked_at
             FROM api_tokens
             ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(tokens)
    }

    pub async fn create_api_token(
        &self,
        name: &str,
        token: &str,
        access_level: ApiTokenAccessLevel,
    ) -> Result<ApiToken> {
        let token_hash = hash_api_token(token);
        let row = sqlx::query_as::<_, ApiToken>(
            "INSERT INTO api_tokens (name, token_hash, access_level)
             VALUES (?, ?, ?)
             RETURNING id, name, access_level, created_at, last_used_at, revoked_at",
        )
        .bind(name)
        .bind(token_hash)
        .bind(access_level)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_active_api_token(&self, token: &str) -> Result<Option<ApiTokenRecord>> {
        let token_hash = hash_api_token(token);
        let row = sqlx::query_as::<_, ApiTokenRecord>(
            "SELECT id, name, token_hash, access_level, created_at, last_used_at, revoked_at
             FROM api_tokens
             WHERE token_hash = ? AND revoked_at IS NULL",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update_api_token_last_used(&self, id: i64) -> Result<()> {
        sqlx::query("UPDATE api_tokens SET last_used_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn revoke_api_token(&self, id: i64) -> Result<()> {
        let result = sqlx::query(
            "UPDATE api_tokens
             SET revoked_at = COALESCE(revoked_at, CURRENT_TIMESTAMP)
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
                    (SELECT COUNT(*) FROM jobs WHERE last_health_check IS NOT NULL AND archived = 0) as total_checked,
                    (SELECT COUNT(*)
                     FROM jobs
                     WHERE health_issues IS NOT NULL AND TRIM(health_issues) != '' AND archived = 0) as issues_found,
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
                        j.input_metadata_json,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::SystemTime;

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
        // Archived jobs are excluded from active stats.
        assert_eq!(aggregated.completed_jobs, 0);
        // encode_stats rows are preserved even after archiving.
        assert_eq!(aggregated.total_input_size, 2_000);
        assert_eq!(aggregated.total_output_size, 1_000);

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
