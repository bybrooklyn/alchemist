use crate::error::Result;
use crate::explanations::{
    Explanation, decision_from_legacy, explanation_from_json, explanation_to_json,
    failure_from_summary,
};
use sqlx::Row;
use std::collections::HashMap;
use std::path::Path;

use super::Db;
use super::timed_query;
use super::types::*;

impl Db {
    pub async fn reset_interrupted_jobs(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE jobs
             SET status = 'queued',
                 progress = 0.0,
                 updated_at = CURRENT_TIMESTAMP
             WHERE status IN ('encoding', 'analyzing', 'remuxing', 'resuming') AND archived = 0",
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
                    created_at, updated_at,
                    input_metadata_json
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
                       created_at, updated_at,
                       input_metadata_json",
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

    pub async fn set_job_input_metadata(
        &self,
        id: i64,
        metadata: &crate::media::pipeline::MediaMetadata,
    ) -> Result<()> {
        let json = serde_json::to_string(metadata)
            .map_err(|e| crate::error::AlchemistError::Unknown(e.to_string()))?;
        sqlx::query("UPDATE jobs SET input_metadata_json = ? WHERE id = ?")
            .bind(json)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn add_decision_with_explanation(
        &self,
        job_id: i64,
        action: &str,
        explanation: &Explanation,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO decisions (job_id, action, reason, reason_code, reason_payload_json)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(job_id)
        .bind(action)
        .bind(&explanation.legacy_reason)
        .bind(&explanation.code)
        .bind(explanation_to_json(explanation))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn add_decision(&self, job_id: i64, action: &str, reason: &str) -> Result<()> {
        let explanation = decision_from_legacy(action, reason);
        self.add_decision_with_explanation(job_id, action, &explanation)
            .await
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
                        j.created_at, j.updated_at, j.input_metadata_json
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

    pub async fn get_duplicate_candidates(&self) -> Result<Vec<DuplicateCandidate>> {
        timed_query("get_duplicate_candidates", || async {
            let all_rows: Vec<DuplicateCandidate> = sqlx::query_as(
                "SELECT id, input_path, status
                     FROM jobs
                     WHERE status NOT IN ('cancelled') AND archived = 0
                     ORDER BY input_path ASC",
            )
            .fetch_all(&self.pool)
            .await?;

            let mut filename_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for row in &all_rows {
                let filename = Path::new(&row.input_path)
                    .file_stem()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if !filename.is_empty() {
                    *filename_counts.entry(filename).or_insert(0) += 1;
                }
            }

            let duplicates = all_rows
                .into_iter()
                .filter(|row| {
                    let filename = Path::new(&row.input_path)
                        .file_stem()
                        .map(|n| n.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    filename_counts.get(&filename).copied().unwrap_or(0) > 1
                })
                .collect();

            Ok(duplicates)
        })
        .await
    }

    pub async fn get_job_decision(&self, job_id: i64) -> Result<Option<Decision>> {
        let decision = sqlx::query_as::<_, Decision>(
            "SELECT id, job_id, action, reason, reason_code, reason_payload_json, created_at
             FROM decisions
             WHERE job_id = ?
             ORDER BY created_at DESC, id DESC
             LIMIT 1",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(decision)
    }

    pub async fn get_job_decision_explanation(&self, job_id: i64) -> Result<Option<Explanation>> {
        let row = sqlx::query_as::<_, DecisionRecord>(
            "SELECT job_id, action, reason, reason_payload_json
             FROM decisions
             WHERE job_id = ?
             ORDER BY created_at DESC, id DESC
             LIMIT 1",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            row.reason_payload_json
                .as_deref()
                .and_then(explanation_from_json)
                .unwrap_or_else(|| decision_from_legacy(&row.action, &row.reason))
        }))
    }

    pub async fn get_job_decision_explanations(
        &self,
        job_ids: &[i64],
    ) -> Result<HashMap<i64, Explanation>> {
        if job_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT d.job_id, d.action, d.reason, d.reason_payload_json
             FROM decisions d
             INNER JOIN (SELECT job_id, MAX(id) AS max_id FROM decisions WHERE job_id IN (",
        );
        let mut separated = qb.separated(", ");
        for job_id in job_ids {
            separated.push_bind(job_id);
        }
        separated.push_unseparated(") GROUP BY job_id) latest ON latest.max_id = d.id");

        let rows = qb
            .build_query_as::<DecisionRecord>()
            .fetch_all(&self.pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let explanation = row
                    .reason_payload_json
                    .as_deref()
                    .and_then(explanation_from_json)
                    .unwrap_or_else(|| decision_from_legacy(&row.action, &row.reason));
                (row.job_id, explanation)
            })
            .collect())
    }

    pub async fn upsert_job_failure_explanation(
        &self,
        job_id: i64,
        explanation: &Explanation,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO job_failure_explanations (job_id, legacy_summary, code, payload_json, updated_at)
             VALUES (?, ?, ?, ?, datetime('now'))
             ON CONFLICT(job_id) DO UPDATE SET
                 legacy_summary = excluded.legacy_summary,
                 code = excluded.code,
                 payload_json = excluded.payload_json,
                 updated_at = datetime('now')",
        )
        .bind(job_id)
        .bind(&explanation.legacy_reason)
        .bind(&explanation.code)
        .bind(explanation_to_json(explanation))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_job_failure_explanation(&self, job_id: i64) -> Result<Option<Explanation>> {
        let row = sqlx::query_as::<_, FailureExplanationRecord>(
            "SELECT legacy_summary, code, payload_json
             FROM job_failure_explanations
             WHERE job_id = ?",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            explanation_from_json(&row.payload_json).unwrap_or_else(|| {
                failure_from_summary(row.legacy_summary.as_deref().unwrap_or(row.code.as_str()))
            })
        }))
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
             SET status = 'queued', progress = 0.0, attempt_count = 0, updated_at = CURRENT_TIMESTAMP
             WHERE status IN ('failed', 'cancelled') AND archived = 0",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Get job by ID
    pub async fn get_job_by_id(&self, id: i64) -> Result<Option<Job>> {
        let job = sqlx::query_as::<_, Job>(
            "SELECT j.id, j.input_path, j.output_path, j.status,
                    (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                    COALESCE(j.priority, 0) as priority,
                    COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                    j.created_at, j.updated_at, j.input_metadata_json
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
                        j.created_at, j.updated_at, j.input_metadata_json
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
                        j.created_at, j.updated_at, j.input_metadata_json
                 FROM jobs j
                 LEFT JOIN encode_stats es ON es.job_id = j.id
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
                let escaped = search
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_");
                qb.push(" AND j.input_path LIKE ");
                qb.push_bind(format!("%{}%", escaped));
                qb.push(" ESCAPE '\\'");
            }

            qb.push(" ORDER BY ");
            let sort_col = match query.sort_by.as_deref() {
                Some("created_at") => "j.created_at",
                Some("updated_at") => "j.updated_at",
                Some("input_path") => "j.input_path",
                Some("size") => "COALESCE(es.input_size_bytes, 0)",
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
            "UPDATE jobs SET status = 'cancelled', updated_at = CURRENT_TIMESTAMP WHERE status IN ('queued', 'analyzing', 'encoding', 'remuxing', 'resuming') AND id IN (",
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
            "UPDATE jobs SET status = 'queued', progress = 0.0, attempt_count = 0, updated_at = CURRENT_TIMESTAMP WHERE id IN (",
        );
        let mut separated = qb.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let result = qb.build().execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn batch_reanalyze_jobs(&self, ids: &[i64]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;

        let mut delete_qb =
            sqlx::QueryBuilder::<sqlx::Sqlite>::new("DELETE FROM decisions WHERE job_id IN (");
        let mut delete_ids = delete_qb.separated(", ");
        for id in ids {
            delete_ids.push_bind(id);
        }
        delete_ids.push_unseparated(")");
        delete_qb.build().execute(&mut *tx).await?;

        let mut delete_resume_qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "DELETE FROM job_resume_sessions WHERE job_id IN (",
        );
        let mut delete_resume_ids = delete_resume_qb.separated(", ");
        for id in ids {
            delete_resume_ids.push_bind(id);
        }
        delete_resume_ids.push_unseparated(")");
        delete_resume_qb.build().execute(&mut *tx).await?;

        let mut delete_stats_qb =
            sqlx::QueryBuilder::<sqlx::Sqlite>::new("DELETE FROM encode_stats WHERE job_id IN (");
        let mut delete_stats_ids = delete_stats_qb.separated(", ");
        for id in ids {
            delete_stats_ids.push_bind(id);
        }
        delete_stats_ids.push_unseparated(")");
        delete_stats_qb.build().execute(&mut *tx).await?;

        let mut update_qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "UPDATE jobs
             SET status = 'queued',
                 progress = 0.0,
                 attempt_count = 0,
                 updated_at = CURRENT_TIMESTAMP
             WHERE archived = 0
               AND id IN (",
        );
        let mut update_ids = update_qb.separated(", ");
        for id in ids {
            update_ids.push_bind(id);
        }
        update_ids.push_unseparated(")");

        let result = update_qb.build().execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result.rows_affected())
    }

    pub async fn purge_jobs_by_filter(
        &self,
        statuses: Option<Vec<JobState>>,
        archived: Option<bool>,
    ) -> Result<u64> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("DELETE FROM jobs WHERE 1=1");

        if let Some(st) = statuses {
            if !st.is_empty() {
                qb.push(" AND status IN (");
                let mut sep = qb.separated(", ");
                for s in st {
                    sep.push_bind(s);
                }
                qb.push(")");
            }
        }

        if let Some(a) = archived {
            qb.push(" AND archived = ");
            qb.push_bind(a);
        }

        let result = qb.build().execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    pub async fn get_jobs_for_intelligence(&self, limit: i64) -> Result<Vec<Job>> {
        let pool = &self.pool;
        timed_query("get_jobs_for_intelligence", || async move {
            let jobs = sqlx::query_as::<_, Job>(
                "SELECT j.id, j.input_path, j.output_path, j.status,
                        (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                        COALESCE(j.priority, 0) as priority,
                        COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                        COALESCE(j.attempt_count, 0) as attempt_count,
                        (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                        j.created_at, j.updated_at, j.input_metadata_json
                 FROM jobs j
                 WHERE j.archived = 0
                   AND j.status != 'cancelled'
                   AND j.input_metadata_json IS NOT NULL
                 ORDER BY j.updated_at DESC
                 LIMIT ?",
            )
            .bind(limit.max(1))
            .fetch_all(pool)
            .await?;
            Ok(jobs)
        })
        .await
    }

    pub async fn get_jobs_under_root_path(&self, root_path: &str) -> Result<Vec<Job>> {
        let jobs = sqlx::query_as::<_, Job>(
            "SELECT j.id, j.input_path, j.output_path, j.status,
                    (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                    COALESCE(j.priority, 0) as priority,
                    COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                    j.created_at, j.updated_at, j.input_metadata_json
             FROM jobs j
             WHERE j.archived = 0
               AND (
                    j.input_path = ?
                    OR (
                        length(j.input_path) > length(?)
                        AND (
                            substr(j.input_path, 1, length(?) + 1) = ? || '/'
                            OR substr(j.input_path, 1, length(?) + 1) = ? || '\\'
                        )
                    )
               )
             ORDER BY j.updated_at DESC",
        )
        .bind(root_path)
        .bind(root_path)
        .bind(root_path)
        .bind(root_path)
        .bind(root_path)
        .bind(root_path)
        .fetch_all(&self.pool)
        .await?;

        Ok(jobs)
    }

    /// Returns the 1-based position of a queued job in the priority queue,
    /// or `None` if the job is not currently queued.
    pub async fn get_queue_position(&self, job_id: i64) -> Result<Option<u32>> {
        let row = sqlx::query(
            "SELECT priority, created_at FROM jobs WHERE id = ? AND status = 'queued' AND archived = 0",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let priority: i64 = row.get("priority");
        let created_at: String = row.get("created_at");

        let pos: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM jobs
             WHERE status = 'queued'
               AND archived = 0
               AND (
                   priority > ?
                   OR (priority = ? AND created_at < ?)
               )",
        )
        .bind(priority)
        .bind(priority)
        .bind(&created_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(Some((pos + 1) as u32))
    }

    pub async fn get_resume_session(&self, job_id: i64) -> Result<Option<JobResumeSession>> {
        let session = sqlx::query_as::<_, JobResumeSession>(
            "SELECT id, job_id, strategy, plan_hash, mtime_hash, temp_dir,
                    concat_manifest_path, segment_length_secs, status, created_at, updated_at
             FROM job_resume_sessions
             WHERE job_id = ?",
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(session)
    }

    pub async fn get_resume_sessions_by_job_ids(
        &self,
        ids: &[i64],
    ) -> Result<Vec<JobResumeSession>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT id, job_id, strategy, plan_hash, mtime_hash, temp_dir,
                    concat_manifest_path, segment_length_secs, status, created_at, updated_at
             FROM job_resume_sessions
             WHERE job_id IN (",
        );
        let mut separated = qb.separated(", ");
        for id in ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");

        let sessions = qb
            .build_query_as::<JobResumeSession>()
            .fetch_all(&self.pool)
            .await?;
        Ok(sessions)
    }

    pub async fn upsert_resume_session(
        &self,
        input: &UpsertJobResumeSessionInput,
    ) -> Result<JobResumeSession> {
        let session = sqlx::query_as::<_, JobResumeSession>(
            "INSERT INTO job_resume_sessions
                (job_id, strategy, plan_hash, mtime_hash, temp_dir,
                 concat_manifest_path, segment_length_secs, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(job_id) DO UPDATE SET
                 strategy = excluded.strategy,
                 plan_hash = excluded.plan_hash,
                 mtime_hash = excluded.mtime_hash,
                 temp_dir = excluded.temp_dir,
                 concat_manifest_path = excluded.concat_manifest_path,
                 segment_length_secs = excluded.segment_length_secs,
                 status = excluded.status,
                 updated_at = CURRENT_TIMESTAMP
             RETURNING id, job_id, strategy, plan_hash, mtime_hash, temp_dir,
                       concat_manifest_path, segment_length_secs, status, created_at, updated_at",
        )
        .bind(input.job_id)
        .bind(&input.strategy)
        .bind(&input.plan_hash)
        .bind(&input.mtime_hash)
        .bind(&input.temp_dir)
        .bind(&input.concat_manifest_path)
        .bind(input.segment_length_secs)
        .bind(&input.status)
        .fetch_one(&self.pool)
        .await?;
        Ok(session)
    }

    pub async fn delete_resume_session(&self, job_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM job_resume_sessions WHERE job_id = ?")
            .bind(job_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_resume_segments(&self, job_id: i64) -> Result<Vec<JobResumeSegment>> {
        let segments = sqlx::query_as::<_, JobResumeSegment>(
            "SELECT id, job_id, segment_index, start_secs, duration_secs,
                    temp_path, status, attempt_count, created_at, updated_at
             FROM job_resume_segments
             WHERE job_id = ?
             ORDER BY segment_index ASC",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(segments)
    }

    pub async fn upsert_resume_segment(
        &self,
        input: &UpsertJobResumeSegmentInput,
    ) -> Result<JobResumeSegment> {
        let segment = sqlx::query_as::<_, JobResumeSegment>(
            "INSERT INTO job_resume_segments
                (job_id, segment_index, start_secs, duration_secs, temp_path, status, attempt_count)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(job_id, segment_index) DO UPDATE SET
                 start_secs = excluded.start_secs,
                 duration_secs = excluded.duration_secs,
                 temp_path = excluded.temp_path,
                 status = excluded.status,
                 attempt_count = excluded.attempt_count,
                 updated_at = CURRENT_TIMESTAMP
             RETURNING id, job_id, segment_index, start_secs, duration_secs,
                       temp_path, status, attempt_count, created_at, updated_at",
        )
        .bind(input.job_id)
        .bind(input.segment_index)
        .bind(input.start_secs)
        .bind(input.duration_secs)
        .bind(&input.temp_path)
        .bind(&input.status)
        .bind(input.attempt_count)
        .fetch_one(&self.pool)
        .await?;
        Ok(segment)
    }

    pub async fn set_resume_segment_status(
        &self,
        job_id: i64,
        segment_index: i64,
        status: &str,
        attempt_count: i32,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE job_resume_segments
             SET status = ?, attempt_count = ?, updated_at = CURRENT_TIMESTAMP
             WHERE job_id = ? AND segment_index = ?",
        )
        .bind(status)
        .bind(attempt_count)
        .bind(job_id)
        .bind(segment_index)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn completed_resume_duration_secs(&self, job_id: i64) -> Result<f64> {
        let duration = sqlx::query_scalar::<_, Option<f64>>(
            "SELECT SUM(duration_secs)
             FROM job_resume_segments
             WHERE job_id = ? AND status = 'completed'",
        )
        .bind(job_id)
        .fetch_one(&self.pool)
        .await?
        .unwrap_or(0.0);
        Ok(duration)
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
                        j.created_at, j.updated_at, j.input_metadata_json
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

    pub async fn get_jobs_for_analysis_batch(&self, offset: i64, limit: i64) -> Result<Vec<Job>> {
        timed_query("get_jobs_for_analysis_batch", || async {
            let rows: Vec<Job> = sqlx::query_as(
                "SELECT j.id, j.input_path, j.output_path,
                        j.status,
                        (SELECT reason FROM decisions
                         WHERE job_id = j.id
                         ORDER BY created_at DESC LIMIT 1)
                         as decision_reason,
                        COALESCE(j.priority, 0) as priority,
                        COALESCE(CAST(j.progress AS REAL),
                                 0.0) as progress,
                        COALESCE(j.attempt_count, 0)
                                 as attempt_count,
                        (SELECT vmaf_score FROM encode_stats
                         WHERE job_id = j.id) as vmaf_score,
                        j.created_at, j.updated_at, j.input_metadata_json
                 FROM jobs j
                 WHERE j.status IN ('queued', 'failed')
                   AND j.archived = 0
                   AND NOT EXISTS (
                       SELECT 1 FROM decisions d
                       WHERE d.job_id = j.id
                   )
                 ORDER BY j.priority DESC, j.created_at ASC
                 LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
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
                    j.created_at, j.updated_at, j.input_metadata_json
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

    pub async fn get_job_by_input_path(&self, path: &str) -> Result<Option<Job>> {
        let job = sqlx::query_as::<_, Job>(
            "SELECT j.id, j.input_path, j.output_path, j.status,
                    (SELECT reason FROM decisions WHERE job_id = j.id ORDER BY created_at DESC LIMIT 1) as decision_reason,
                    COALESCE(j.priority, 0) as priority,
                    COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                    COALESCE(j.attempt_count, 0) as attempt_count,
                    (SELECT vmaf_score FROM encode_stats WHERE job_id = j.id) as vmaf_score,
                    j.created_at, j.updated_at, j.input_metadata_json
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
            sqlx::query_as("SELECT 1 FROM jobs WHERE output_path = ? AND archived = 0 LIMIT 1")
                .bind(path)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
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
                        j.created_at, j.updated_at, j.input_metadata_json
                 FROM jobs j
                 WHERE j.status = 'completed'
                   AND j.archived = 0
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;
    use std::time::SystemTime;

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
    async fn claim_next_job_handles_queue_spam_without_duplicates()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_queue_spam_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let job_count = 128;

        for index in 0..job_count {
            let input = format!("spam-input-{index:03}.mkv");
            let output = format!("spam-output-{index:03}.mkv");
            let changed = db
                .enqueue_job(
                    Path::new(&input),
                    Path::new(&output),
                    SystemTime::UNIX_EPOCH,
                )
                .await?;
            assert!(changed, "expected fresh insert for {input}");
        }

        let mut claimed_ids = HashSet::new();
        let mut claimed_inputs = HashSet::new();
        for _ in 0..job_count {
            let claimed = db
                .claim_next_job()
                .await?
                .ok_or_else(|| std::io::Error::other("queue drained before every job claimed"))?;
            assert_eq!(claimed.status, JobState::Analyzing);
            assert!(
                claimed_ids.insert(claimed.id),
                "job {} was claimed more than once",
                claimed.id
            );
            assert!(
                claimed_inputs.insert(claimed.input_path.clone()),
                "input {} was claimed more than once",
                claimed.input_path
            );
        }

        assert!(db.claim_next_job().await?.is_none());
        assert_eq!(claimed_ids.len(), job_count);
        assert!(db.get_jobs_by_status(JobState::Queued).await?.is_empty());
        assert_eq!(
            db.get_jobs_by_status(JobState::Analyzing).await?.len(),
            job_count
        );

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
    async fn reset_interrupted_jobs_requeues_only_interrupted_states()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_reset_interrupted_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        let jobs = [
            ("queued.mkv", "queued-out.mkv", JobState::Queued),
            ("analyzing.mkv", "analyzing-out.mkv", JobState::Analyzing),
            ("encoding.mkv", "encoding-out.mkv", JobState::Encoding),
            ("remuxing.mkv", "remuxing-out.mkv", JobState::Remuxing),
            ("cancelled.mkv", "cancelled-out.mkv", JobState::Cancelled),
            ("completed.mkv", "completed-out.mkv", JobState::Completed),
        ];

        for (input, output, status) in jobs {
            let _ = db
                .enqueue_job(Path::new(input), Path::new(output), SystemTime::UNIX_EPOCH)
                .await?;
            let job = db
                .get_job_by_input_path(input)
                .await?
                .ok_or_else(|| std::io::Error::other("missing seeded job"))?;
            db.update_job_status(job.id, status).await?;
        }

        let reset = db.reset_interrupted_jobs().await?;
        assert_eq!(reset, 3);

        assert_eq!(
            db.get_job_by_input_path("analyzing.mkv")
                .await?
                .ok_or_else(|| std::io::Error::other("missing analyzing job"))?
                .status,
            JobState::Queued
        );
        assert_eq!(
            db.get_job_by_input_path("encoding.mkv")
                .await?
                .ok_or_else(|| std::io::Error::other("missing encoding job"))?
                .status,
            JobState::Queued
        );
        assert_eq!(
            db.get_job_by_input_path("remuxing.mkv")
                .await?
                .ok_or_else(|| std::io::Error::other("missing remuxing job"))?
                .status,
            JobState::Queued
        );
        assert_eq!(
            db.get_job_by_input_path("cancelled.mkv")
                .await?
                .ok_or_else(|| std::io::Error::other("missing cancelled job"))?
                .status,
            JobState::Cancelled
        );
        assert_eq!(
            db.get_job_by_input_path("completed.mkv")
                .await?
                .ok_or_else(|| std::io::Error::other("missing completed job"))?
                .status,
            JobState::Completed
        );

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn get_jobs_needing_health_check_excludes_archived_rows()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_health_check_jobs_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let input = Path::new("health-input.mkv");
        let output = Path::new("health-output.mkv");
        let _ = db
            .enqueue_job(input, output, SystemTime::UNIX_EPOCH)
            .await?;

        let job = db
            .get_job_by_input_path("health-input.mkv")
            .await?
            .ok_or_else(|| std::io::Error::other("missing health job"))?;
        db.update_job_status(job.id, JobState::Completed).await?;
        db.batch_delete_jobs(&[job.id]).await?;

        let jobs = db.get_jobs_needing_health_check().await?;
        assert!(jobs.is_empty());

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn legacy_decision_rows_still_parse_into_structured_explanations()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_legacy_decision_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        let _ = db
            .enqueue_job(
                Path::new("legacy-input.mkv"),
                Path::new("legacy-output.mkv"),
                SystemTime::UNIX_EPOCH,
            )
            .await?;
        let job = db
            .get_job_by_input_path("legacy-input.mkv")
            .await?
            .ok_or_else(|| std::io::Error::other("missing job"))?;

        sqlx::query(
            "INSERT INTO decisions (job_id, action, reason, reason_code, reason_payload_json)
             VALUES (?, 'skip', 'bpp_below_threshold|bpp=0.043,threshold=0.050', NULL, NULL)",
        )
        .bind(job.id)
        .execute(&db.pool)
        .await?;

        let explanation = db
            .get_job_decision_explanation(job.id)
            .await?
            .ok_or_else(|| std::io::Error::other("missing explanation"))?;
        assert_eq!(explanation.code, "bpp_below_threshold");
        assert_eq!(
            explanation.measured.get("bpp"),
            Some(&serde_json::json!(0.043))
        );

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
