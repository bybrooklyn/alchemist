use crate::error::Result;

use super::Db;
use super::types::*;

impl Db {
    pub async fn create_conversion_job(
        &self,
        upload_path: &str,
        mode: &str,
        settings_json: &str,
        probe_json: Option<&str>,
        expires_at: &str,
    ) -> Result<ConversionJob> {
        let row = sqlx::query_as::<_, ConversionJob>(
            "INSERT INTO conversion_jobs (upload_path, mode, settings_json, probe_json, expires_at)
             VALUES (?, ?, ?, ?, ?)
             RETURNING *",
        )
        .bind(upload_path)
        .bind(mode)
        .bind(settings_json)
        .bind(probe_json)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_conversion_job(&self, id: i64) -> Result<Option<ConversionJob>> {
        let row = sqlx::query_as::<_, ConversionJob>(
            "SELECT id, upload_path, output_path, mode, settings_json, probe_json, linked_job_id, status, expires_at, downloaded_at, created_at, updated_at
             FROM conversion_jobs
             WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_conversion_job_by_linked_job_id(
        &self,
        linked_job_id: i64,
    ) -> Result<Option<ConversionJob>> {
        let row = sqlx::query_as::<_, ConversionJob>(
            "SELECT id, upload_path, output_path, mode, settings_json, probe_json, linked_job_id, status, expires_at, downloaded_at, created_at, updated_at
             FROM conversion_jobs
             WHERE linked_job_id = ?",
        )
        .bind(linked_job_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn update_conversion_job_probe(&self, id: i64, probe_json: &str) -> Result<()> {
        sqlx::query(
            "UPDATE conversion_jobs
             SET probe_json = ?, updated_at = datetime('now')
             WHERE id = ?",
        )
        .bind(probe_json)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_conversion_job_settings(
        &self,
        id: i64,
        settings_json: &str,
        mode: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE conversion_jobs
             SET settings_json = ?, mode = ?, updated_at = datetime('now')
             WHERE id = ?",
        )
        .bind(settings_json)
        .bind(mode)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn persist_conversion_job_preview(
        &self,
        id: i64,
        settings_json: &str,
        mode: &str,
        status: &str,
        probe_json: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE conversion_jobs
             SET settings_json = ?,
                 mode = ?,
                 status = ?,
                 probe_json = ?,
                 updated_at = datetime('now')
             WHERE id = ?",
        )
        .bind(settings_json)
        .bind(mode)
        .bind(status)
        .bind(probe_json)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_conversion_job_start(
        &self,
        id: i64,
        output_path: &str,
        linked_job_id: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE conversion_jobs
             SET output_path = ?, linked_job_id = ?, status = 'queued', updated_at = datetime('now')
             WHERE id = ?",
        )
        .bind(output_path)
        .bind(linked_job_id)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_conversion_job_status(&self, id: i64, status: &str) -> Result<()> {
        sqlx::query(
            "UPDATE conversion_jobs
             SET status = ?, updated_at = datetime('now')
             WHERE id = ?",
        )
        .bind(status)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_conversion_job_downloaded(&self, id: i64, expires_at: &str) -> Result<()> {
        sqlx::query(
            "UPDATE conversion_jobs
             SET downloaded_at = datetime('now'),
                 expires_at = ?,
                 status = 'downloaded',
                 updated_at = datetime('now')
             WHERE id = ?",
        )
        .bind(expires_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_conversion_job(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM conversion_jobs WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_conversion_jobs_ready_for_cleanup(
        &self,
        now: &str,
    ) -> Result<Vec<ConversionJob>> {
        let rows = sqlx::query_as::<_, ConversionJob>(
            "SELECT cj.id, cj.upload_path, cj.output_path, cj.mode, cj.settings_json, cj.probe_json,
                    cj.linked_job_id, cj.status, cj.expires_at, cj.downloaded_at, cj.created_at,
                    cj.updated_at
             FROM conversion_jobs cj
             LEFT JOIN jobs j ON j.id = cj.linked_job_id
             WHERE (
                    cj.linked_job_id IS NULL
                    AND datetime(cj.expires_at) <= datetime(?)
                   )
                OR (
                    cj.linked_job_id IS NOT NULL
                    AND j.id IS NULL
                    AND datetime(cj.expires_at) <= datetime(?)
                   )
                OR (
                    j.archived = 1
                    AND datetime(cj.expires_at) <= datetime(?)
                   )
                OR (
                    j.status IN ('failed', 'cancelled', 'skipped')
                    AND j.archived = 0
                    AND datetime(j.updated_at) <= datetime(?, '-24 hours')
                   )
                OR (
                    j.status = 'completed'
                    AND cj.downloaded_at IS NOT NULL
                    AND datetime(cj.expires_at) <= datetime(?)
                   )",
        )
        .bind(now)
        .bind(now)
        .bind(now)
        .bind(now)
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
