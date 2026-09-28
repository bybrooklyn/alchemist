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

/// Upsert used by every enqueue path (single watcher insert and batched scan
/// insert). Keeping it in one place ensures the `ON CONFLICT` change-detection
/// semantics never drift between the two. Bind order:
/// `(input_path, output_path, mtime_hash, source_device)`.
const ENQUEUE_JOB_UPSERT_SQL: &str =
    "INSERT INTO jobs (input_path, output_path, status, mtime_hash, source_device, updated_at)
     VALUES (?, ?, 'queued', ?, ?, CURRENT_TIMESTAMP)
     ON CONFLICT(input_path) DO UPDATE SET
     output_path = excluded.output_path,
     status = CASE WHEN mtime_hash != excluded.mtime_hash THEN 'queued' ELSE status END,
     archived = 0,
     mtime_hash = excluded.mtime_hash,
     source_device = excluded.source_device,
     updated_at = CURRENT_TIMESTAMP
     WHERE mtime_hash != excluded.mtime_hash OR output_path != excluded.output_path";

/// Maximum job ids bound into a single `IN (...)` clause.
///
/// SQLite's `SQLITE_MAX_VARIABLE_NUMBER` defaults to 32766, and exceeding it
/// fails the entire statement with "too many SQL variables". That is reachable
/// in ordinary use, not just under abuse: `reanalyze_library_root_handler`
/// collects *every* non-active job under a watch folder and hands the whole list
/// to `batch_reanalyze_jobs`, so any library past ~32k files could never be
/// re-analyzed. Every id-list query chunks through this instead.
const ID_CHUNK: usize = 500;

impl Db {
    /// Requeue every job left mid-flight by an unclean shutdown. Deliberately
    /// includes archived rows: nothing can still be running them after a
    /// restart, and an archived job stuck in an active state is exactly the
    /// state that deadlocks Balanced-mode device exclusion (a stale
    /// 'analyzing' row keeps excluding its whole device forever, since
    /// nothing else will ever move it out of that state). They stay
    /// archived, so `archived = 0` filters elsewhere still keep them out of
    /// every claim and analysis query — resetting their status here only
    /// stops them from poisoning the active-device set.
    pub async fn reset_interrupted_jobs(&self) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE jobs
             SET status = 'queued',
                 progress = 0.0,
                 updated_at = CURRENT_TIMESTAMP
             WHERE status IN ('encoding', 'analyzing', 'remuxing', 'resuming')",
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

        let mtime_hash = mtime_hash_string(mtime);
        let source_device = crate::system::device_id::device_id_for_async(input_path).await;

        let result = sqlx::query(ENQUEUE_JOB_UPSERT_SQL)
            .bind(input_str)
            .bind(output_str)
            .bind(mtime_hash)
            .bind(source_device)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Insert/upsert a batch of already-resolved jobs inside a single
    /// transaction. The library scan path resolves discovered files (skip
    /// checks, output paths, device ids) up front, then writes them here in
    /// chunks — so a large scan no longer issues thousands of individual write
    /// transactions that monopolize the shared connection pool. Returns the
    /// number of rows actually inserted or updated, matching `enqueue_job`'s
    /// `changed` semantics (no-op upserts are not counted).
    pub async fn enqueue_jobs_batch(&self, jobs: &[PreparedEnqueue]) -> Result<u64> {
        if jobs.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut changed: u64 = 0;
        for job in jobs {
            let result = sqlx::query(ENQUEUE_JOB_UPSERT_SQL)
                .bind(&job.input_path)
                .bind(&job.output_path)
                .bind(&job.mtime_hash)
                .bind(&job.source_device)
                .execute(&mut *tx)
                .await?;
            changed += result.rows_affected();
        }
        tx.commit().await?;

        Ok(changed)
    }

    pub async fn add_job(&self, job: Job) -> Result<()> {
        sqlx::query(
            "INSERT INTO jobs (input_path, output_path, status, mtime_hash, priority, progress, attempt_count, source_device, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(job.input_path)
        .bind(job.output_path)
        .bind(job.status)
        .bind("0.0")
        .bind(job.priority)
        .bind(job.progress)
        .bind(job.attempt_count)
        .bind(job.source_device)
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
                    input_metadata_json, source_device
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
        self.claim_next_job_with_mode(crate::config::EngineMode::Throughput)
            .await
    }

    /// Claim the next queued job, optionally excluding candidates that share a
    /// `source_device` with an already-running job.
    ///
    /// In `Balanced` mode the partial index `idx_jobs_source_device_active`
    /// supports finding which devices currently have an active job; the
    /// claim query then skips queued rows whose `source_device` matches.
    /// `null` source_device counts as its own unique device — multiple
    /// unknown-device jobs can run concurrently (matches pre-feature
    /// behavior).
    ///
    /// `Throughput` and `Background` modes do not perform device grouping;
    /// concurrency is governed solely by the processor's semaphore.
    pub async fn claim_next_job_with_mode(
        &self,
        mode: crate::config::EngineMode,
    ) -> Result<Option<Job>> {
        let active_states = "('analyzing', 'encoding', 'remuxing', 'resuming')";
        let device_exclusion = match mode {
            crate::config::EngineMode::Balanced => format!(
                "AND (
                    source_device IS NULL
                    OR source_device NOT IN (
                        SELECT source_device FROM jobs
                        WHERE status IN {} AND source_device IS NOT NULL AND archived = 0
                    )
                 )",
                active_states
            ),
            _ => String::new(),
        };

        let sql = format!(
            "UPDATE jobs
             SET status = 'analyzing', updated_at = CURRENT_TIMESTAMP
             WHERE id = (
                 SELECT id
                 FROM jobs
                 WHERE status = 'queued'
                   AND archived = 0
                   {}
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
                       input_metadata_json, source_device",
            device_exclusion,
        );

        let job = sqlx::query_as::<_, Job>(&sql)
            .fetch_optional(&self.pool)
            .await?;

        Ok(job)
    }

    /// By-id status update used by every stage of the pipeline (claim,
    /// analyze, encode, finish, cancel, ...). Moving a job INTO an active
    /// state (Analyzing/Encoding/Remuxing/Resuming) additionally requires
    /// `archived = 0`: an archived job in an active state is never claimed
    /// again (nothing consumes it), so it would sit there forever and, in
    /// Balanced mode, keep excluding its whole device from claiming. Every
    /// other target status is unaffected — an archived job still needs to
    /// be settable to Completed/Failed/Cancelled/Skipped/Queued so it can
    /// be closed out cleanly.
    ///
    /// If the row exists but is archived and `status` is an active state,
    /// this is a deliberate no-op (`Ok(())`) rather than an error: the row
    /// was found, it just isn't eligible for that transition. A genuinely
    /// missing id still returns `RowNotFound`, as before.
    pub async fn update_job_status(&self, id: i64, status: JobState) -> Result<()> {
        let guard_archived = matches!(
            status,
            JobState::Analyzing | JobState::Encoding | JobState::Remuxing | JobState::Resuming
        );

        let result = if guard_archived {
            sqlx::query(
                "UPDATE jobs SET status = ?, updated_at = CURRENT_TIMESTAMP
                 WHERE id = ? AND archived = 0",
            )
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?
        } else {
            sqlx::query("UPDATE jobs SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(status)
                .bind(id)
                .execute(&self.pool)
                .await?
        };

        if result.rows_affected() == 0 {
            if guard_archived {
                let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM jobs WHERE id = ?")
                    .bind(id)
                    .fetch_optional(&self.pool)
                    .await?;
                if exists.is_some() {
                    // Row exists, just archived — not an error.
                    return Ok(());
                }
            }
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
        let mut out = HashMap::new();
        for chunk in job_ids.chunks(ID_CHUNK) {
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "SELECT d.job_id, d.action, d.reason, d.reason_payload_json
                 FROM decisions d
                 INNER JOIN (SELECT job_id, MAX(id) AS max_id FROM decisions WHERE job_id IN (",
            );
            let mut separated = qb.separated(", ");
            for job_id in chunk {
                separated.push_bind(job_id);
            }
            separated.push_unseparated(") GROUP BY job_id) latest ON latest.max_id = d.id");

            let rows = qb
                .build_query_as::<DecisionRecord>()
                .fetch_all(&self.pool)
                .await?;

            out.extend(rows.into_iter().map(|row| {
                let explanation = row
                    .reason_payload_json
                    .as_deref()
                    .and_then(explanation_from_json)
                    .unwrap_or_else(|| decision_from_legacy(&row.action, &row.reason));
                (row.job_id, explanation)
            }));
        }
        Ok(out)
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

    pub async fn get_job_failure_explanations(
        &self,
        job_ids: &[i64],
    ) -> Result<HashMap<i64, Explanation>> {
        let mut out = HashMap::new();
        for chunk in job_ids.chunks(ID_CHUNK) {
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "SELECT job_id, legacy_summary, code, payload_json
                 FROM job_failure_explanations
                 WHERE job_id IN (",
            );
            let mut separated = qb.separated(", ");
            for job_id in chunk {
                separated.push_bind(job_id);
            }
            separated.push_unseparated(")");

            let rows = qb
                .build_query_as::<JobFailureExplanationRecord>()
                .fetch_all(&self.pool)
                .await?;

            out.extend(rows.into_iter().map(|row| {
                let explanation = explanation_from_json(&row.payload_json).unwrap_or_else(|| {
                    failure_from_summary(row.legacy_summary.as_deref().unwrap_or(row.code.as_str()))
                });
                (row.job_id, explanation)
            }));
        }
        Ok(out)
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
                    j.created_at, j.updated_at, j.input_metadata_json, j.source_device
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
                        j.created_at, j.updated_at, j.input_metadata_json, j.source_device
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
                        j.created_at, j.updated_at, j.input_metadata_json, j.source_device
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

            if let Some(ref statuses) = query.statuses
                && !statuses.is_empty() {
                    qb.push(" AND j.status IN (");
                    let mut separated = qb.separated(", ");
                    for status in statuses {
                        separated.push_bind(*status);
                    }
                    separated.push_unseparated(") ");
                }

            if let Some(ref search) = query.search {
                let escaped = search
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_");
                let pattern = format!("%{}%", escaped);
                qb.push(" AND (j.input_path LIKE ");
                qb.push_bind(pattern.clone());
                qb.push(" ESCAPE '\\'");
                qb.push(
                    " OR EXISTS (
                        SELECT 1 FROM decisions d
                        WHERE d.job_id = j.id
                          AND (
                            d.reason LIKE ",
                );
                qb.push_bind(pattern.clone());
                qb.push(" ESCAPE '\\' OR COALESCE(d.reason_code, d.action, '') LIKE ");
                qb.push_bind(pattern.clone());
                qb.push(" ESCAPE '\\' OR COALESCE(d.reason_payload_json, '') LIKE ");
                qb.push_bind(pattern.clone());
                qb.push(
                    " ESCAPE '\\'
                          )
                    ) OR EXISTS (
                        SELECT 1 FROM job_failure_explanations f
                        WHERE f.job_id = j.id
                          AND (
                            COALESCE(f.legacy_summary, '') LIKE ",
                );
                qb.push_bind(pattern.clone());
                qb.push(" ESCAPE '\\' OR f.code LIKE ");
                qb.push_bind(pattern.clone());
                qb.push(" ESCAPE '\\' OR f.payload_json LIKE ");
                qb.push_bind(pattern);
                qb.push(
                    " ESCAPE '\\'
                          )
                    )
                ) ",
                );
            }

            if let Some(ref reason_code) = query.reason_code {
                qb.push(
                    " AND EXISTS (
                        SELECT 1 FROM decisions d
                        WHERE d.job_id = j.id
                          AND COALESCE(d.reason_code, d.action) = ",
                );
                qb.push_bind(reason_code.clone());
                qb.push(") ");
            }

            if let Some(ref failure_code) = query.failure_code {
                qb.push(
                    " AND EXISTS (
                        SELECT 1 FROM job_failure_explanations f
                        WHERE f.job_id = j.id AND f.code = ",
                );
                qb.push_bind(failure_code.clone());
                qb.push(") ");
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
        let mut affected = 0_u64;
        for chunk in ids.chunks(ID_CHUNK) {
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "UPDATE jobs SET status = 'cancelled', updated_at = CURRENT_TIMESTAMP WHERE status IN ('queued', 'analyzing', 'encoding', 'remuxing', 'resuming') AND id IN (",
            );
            let mut separated = qb.separated(", ");
            for id in chunk {
                separated.push_bind(id);
            }
            separated.push_unseparated(")");

            affected += qb.build().execute(&self.pool).await?.rows_affected();
        }
        Ok(affected)
    }

    pub async fn batch_delete_jobs(&self, ids: &[i64]) -> Result<u64> {
        let mut affected = 0_u64;
        for chunk in ids.chunks(ID_CHUNK) {
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "UPDATE jobs SET archived = 1, updated_at = CURRENT_TIMESTAMP WHERE archived = 0 AND status NOT IN ('analyzing', 'encoding', 'remuxing', 'resuming') AND id IN (",
            );
            let mut separated = qb.separated(", ");
            for id in chunk {
                separated.push_bind(id);
            }
            separated.push_unseparated(")");

            affected += qb.build().execute(&self.pool).await?.rows_affected();
        }
        Ok(affected)
    }

    pub async fn batch_restart_jobs(&self, ids: &[i64]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut affected = 0_u64;
        for chunk in ids.chunks(ID_CHUNK) {
            // Clear the stale failure explanation as part of the restart. Without
            // this, a job requeued after a failure kept rendering its previous
            // failure banner in the jobs table and detail modal even while it sat
            // healthy in the queue. Scoped to the same rows the UPDATE below will
            // actually touch, so an ineligible id never loses its explanation.
            let mut clear_qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "DELETE FROM job_failure_explanations WHERE job_id IN (
                     SELECT id FROM jobs
                     WHERE archived = 0
                       AND status NOT IN ('analyzing', 'encoding', 'remuxing', 'resuming')
                       AND id IN (",
            );
            let mut clear_ids = clear_qb.separated(", ");
            for id in chunk {
                clear_ids.push_bind(id);
            }
            clear_ids.push_unseparated("))");
            clear_qb.build().execute(&mut *tx).await?;

            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "UPDATE jobs SET status = 'queued', progress = 0.0, attempt_count = 0, updated_at = CURRENT_TIMESTAMP WHERE archived = 0 AND status NOT IN ('analyzing', 'encoding', 'remuxing', 'resuming') AND id IN (",
            );
            let mut separated = qb.separated(", ");
            for id in chunk {
                separated.push_bind(id);
            }
            separated.push_unseparated(")");

            affected += qb.build().execute(&mut *tx).await?.rows_affected();
        }
        tx.commit().await?;
        Ok(affected)
    }

    pub async fn batch_reanalyze_jobs(&self, ids: &[i64]) -> Result<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool.begin().await?;
        let mut affected = 0_u64;

        for chunk in ids.chunks(ID_CHUNK) {
            // A reanalyze wipes every derived record for the job. The failure
            // explanation is one of them: leaving it behind meant a job that was
            // re-analyzed clean still showed its old failure in the UI.
            for table in [
                "decisions",
                "job_resume_sessions",
                "encode_stats",
                "job_failure_explanations",
            ] {
                let mut delete_qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(format!(
                    "DELETE FROM {table} WHERE job_id IN ("
                ));
                let mut delete_ids = delete_qb.separated(", ");
                for id in chunk {
                    delete_ids.push_bind(id);
                }
                delete_ids.push_unseparated(")");
                delete_qb.build().execute(&mut *tx).await?;
            }

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
            for id in chunk {
                update_ids.push_bind(id);
            }
            update_ids.push_unseparated(")");

            affected += update_qb.build().execute(&mut *tx).await?.rows_affected();
        }

        tx.commit().await?;
        Ok(affected)
    }

    pub async fn reanalyze_jobs_under_path(&self, root_path: &str) -> Result<u64> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("CREATE TEMPORARY TABLE jobs_to_reanalyze (id INTEGER PRIMARY KEY)")
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "INSERT INTO jobs_to_reanalyze (id)
             SELECT id FROM jobs
             WHERE archived = 0
               AND status NOT IN ('analyzing', 'encoding', 'remuxing', 'resuming')
               AND (
                    input_path = ?
                    OR (
                        length(input_path) > length(?)
                        AND (
                            substr(input_path, 1, length(?) + 1) = ? || '/'
                            OR substr(input_path, 1, length(?) + 1) = ? || '\\'
                        )
                    )
               )",
        )
        .bind(root_path)
        .bind(root_path)
        .bind(root_path)
        .bind(root_path)
        .bind(root_path)
        .bind(root_path)
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM decisions WHERE job_id IN (SELECT id FROM jobs_to_reanalyze)")
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            "DELETE FROM job_resume_sessions WHERE job_id IN (SELECT id FROM jobs_to_reanalyze)",
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM encode_stats WHERE job_id IN (SELECT id FROM jobs_to_reanalyze)")
            .execute(&mut *tx)
            .await?;

        // Matches `batch_reanalyze_jobs`: a reanalyze clears every derived record,
        // including the failure explanation, so a job that re-analyzes clean stops
        // showing its previous failure.
        sqlx::query(
            "DELETE FROM job_failure_explanations WHERE job_id IN (SELECT id FROM jobs_to_reanalyze)",
        )
        .execute(&mut *tx)
        .await?;

        let result = sqlx::query(
            "UPDATE jobs
             SET status = 'queued',
                 progress = 0.0,
                 attempt_count = 0,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id IN (SELECT id FROM jobs_to_reanalyze)",
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query("DROP TABLE jobs_to_reanalyze")
            .execute(&mut *tx)
            .await?;

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
            if st.is_empty() {
                // Caller asked for a status filter but supplied none we could
                // recognise. Refuse to broaden the delete instead of falling
                // back to WHERE 1=1, which would wipe every job in the table.
                return Ok(0);
            }
            qb.push(" AND status IN (");
            let mut sep = qb.separated(", ");
            for s in st {
                sep.push_bind(s);
            }
            qb.push(")");
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
                        j.created_at, j.updated_at, j.input_metadata_json, j.source_device
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
                    j.created_at, j.updated_at, j.input_metadata_json, j.source_device
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
        let mut sessions = Vec::new();
        for chunk in ids.chunks(ID_CHUNK) {
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "SELECT id, job_id, strategy, plan_hash, mtime_hash, temp_dir,
                        concat_manifest_path, segment_length_secs, status, created_at, updated_at
                 FROM job_resume_sessions
                 WHERE job_id IN (",
            );
            let mut separated = qb.separated(", ");
            for id in chunk {
                separated.push_bind(id);
            }
            separated.push_unseparated(")");

            sessions.extend(
                qb.build_query_as::<JobResumeSession>()
                    .fetch_all(&self.pool)
                    .await?,
            );
        }
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
                        j.created_at, j.updated_at, j.input_metadata_json, j.source_device
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
                        j.created_at, j.updated_at, j.input_metadata_json, j.source_device
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

    /// Keyset-paginated variant of `get_jobs_for_analysis_batch` for the
    /// auto-analysis pass. `after` is the `(priority, created_at, id)` of
    /// the last row the caller saw, in the same sort order used here
    /// (priority DESC, created_at ASC, id ASC as the final tie-breaker);
    /// `None` fetches the first page. Unlike OFFSET, a row leaving the
    /// result set between pages (because analysis gave it a decision)
    /// cannot cause this to skip or repeat rows, since the next page is
    /// derived only from the last row's own sort key, not from how many
    /// earlier rows still match. `COALESCE(j.priority, 0)` is used on both
    /// sides of the comparison and in ORDER BY so the two stay consistent
    /// even for legacy rows with a NULL priority.
    pub async fn get_jobs_for_analysis_batch_after(
        &self,
        after: Option<(i32, chrono::DateTime<chrono::Utc>, i64)>,
        limit: i64,
    ) -> Result<Vec<Job>> {
        timed_query("get_jobs_for_analysis_batch_after", || async {
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "SELECT j.id, j.input_path, j.output_path,
                        j.status,
                        (SELECT reason FROM decisions
                         WHERE job_id = j.id
                         ORDER BY created_at DESC LIMIT 1)
                         as decision_reason,
                        COALESCE(j.priority, 0) as priority,
                        COALESCE(CAST(j.progress AS REAL), 0.0) as progress,
                        COALESCE(j.attempt_count, 0) as attempt_count,
                        (SELECT vmaf_score FROM encode_stats
                         WHERE job_id = j.id) as vmaf_score,
                        j.created_at, j.updated_at, j.input_metadata_json, j.source_device
                 FROM jobs j
                 WHERE j.status IN ('queued', 'failed')
                   AND j.archived = 0
                   AND NOT EXISTS (
                       SELECT 1 FROM decisions d
                       WHERE d.job_id = j.id
                   )",
            );

            // `created_at` is stored as SQLite TEXT and, depending on
            // insert path, can be either the schema's own
            // `CURRENT_TIMESTAMP` rendering or a chrono-encoded value with
            // a different (but equally valid) textual form. Comparing and
            // ordering through SQLite's `datetime()` normalizes both to
            // the same canonical form, so a plain string `=`/`>` on mixed
            // formats can't silently mismatch and drop the cursor off the
            // end of the result set.
            if let Some((priority, created_at, id)) = after {
                qb.push(" AND (COALESCE(j.priority, 0) < ")
                    .push_bind(priority)
                    .push(" OR (COALESCE(j.priority, 0) = ")
                    .push_bind(priority)
                    .push(" AND datetime(j.created_at) > datetime(")
                    .push_bind(created_at)
                    .push(")) OR (COALESCE(j.priority, 0) = ")
                    .push_bind(priority)
                    .push(" AND datetime(j.created_at) = datetime(")
                    .push_bind(created_at)
                    .push(") AND j.id > ")
                    .push_bind(id)
                    .push("))");
            }

            qb.push(
                " ORDER BY COALESCE(j.priority, 0) DESC, datetime(j.created_at) ASC, j.id ASC LIMIT ",
            )
            .push_bind(limit);

            let rows: Vec<Job> = qb.build_query_as().fetch_all(&self.pool).await?;
            Ok(rows)
        })
        .await
    }

    pub async fn get_jobs_by_ids(&self, ids: &[i64]) -> Result<Vec<Job>> {
        let mut jobs = Vec::new();
        for chunk in ids.chunks(ID_CHUNK) {
            jobs.extend(self.get_jobs_by_ids_chunk(chunk).await?);
        }
        // Each chunk is ordered on its own, so re-sort once across chunks to keep
        // the documented newest-first ordering for callers.
        jobs.sort_by_key(|job| std::cmp::Reverse(job.updated_at));
        Ok(jobs)
    }

    async fn get_jobs_by_ids_chunk(&self, ids: &[i64]) -> Result<Vec<Job>> {
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
                    j.created_at, j.updated_at, j.input_metadata_json, j.source_device
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
                    j.created_at, j.updated_at, j.input_metadata_json, j.source_device
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
                        j.created_at, j.updated_at, j.input_metadata_json, j.source_device
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
             WHERE id = ?
               AND archived = 0
               AND status NOT IN ('analyzing', 'encoding', 'remuxing', 'resuming')",
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

    /// Keyset pagination over the analysis selection: consecutive pages
    /// are disjoint and jointly complete, and a row that gains a decision
    /// between pages shifts neither page (the cursor derives from the
    /// last row's sort key, not from an offset).
    #[tokio::test]
    async fn analysis_batch_keyset_pages_are_disjoint_and_complete()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_analysis_keyset_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        for name in ["keyset-a.mkv", "keyset-b.mkv", "keyset-c.mkv"] {
            let out = name.replace(".mkv", "-out.mkv");
            let changed = db
                .enqueue_job(Path::new(name), Path::new(&out), SystemTime::UNIX_EPOCH)
                .await?;
            assert!(changed, "expected fresh insert for {name}");
        }

        let page1 = db.get_jobs_for_analysis_batch_after(None, 2).await?;
        assert_eq!(page1.len(), 2);
        let last = page1
            .last()
            .ok_or_else(|| std::io::Error::other("empty first page"))?;
        let cursor = Some((last.priority, last.created_at, last.id));

        // A decision on a first-page row between fetches must not shift
        // the second page: it simply drops out of the result set.
        sqlx::query("INSERT INTO decisions (job_id, action, reason) VALUES (?, 'skip', 'test')")
            .bind(page1[0].id)
            .execute(&db.pool)
            .await?;

        let page2 = db.get_jobs_for_analysis_batch_after(cursor, 2).await?;
        assert_eq!(page2.len(), 1);

        let mut seen: HashSet<i64> = HashSet::new();
        for job in page1.iter().chain(page2.iter()) {
            assert!(seen.insert(job.id), "job {} repeated across pages", job.id);
        }
        // page1[0] earned a decision, so the live selection now holds the
        // other two rows — both must have been visited exactly once.
        assert_eq!(seen.len(), 3);

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
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
    async fn claim_next_job_balanced_mode_excludes_in_flight_devices()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_balanced_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        for i in 0..2 {
            db.add_job(Job {
                id: 0,
                input_path: format!("/disk-a/file{}.mkv", i),
                output_path: format!("/disk-a/file{}.out.mkv", i),
                status: JobState::Queued,
                decision_reason: None,
                priority: 0,
                progress: 0.0,
                attempt_count: 0,
                vmaf_score: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                input_metadata_json: None,
                source_device: Some("dev:42".to_string()),
            })
            .await?;
        }
        for i in 0..2 {
            db.add_job(Job {
                id: 0,
                input_path: format!("/disk-b/file{}.mkv", i),
                output_path: format!("/disk-b/file{}.out.mkv", i),
                status: JobState::Queued,
                decision_reason: None,
                priority: 0,
                progress: 0.0,
                attempt_count: 0,
                vmaf_score: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                input_metadata_json: None,
                source_device: Some("dev:99".to_string()),
            })
            .await?;
        }

        let first = db
            .claim_next_job_with_mode(crate::config::EngineMode::Balanced)
            .await?
            .ok_or_else(|| std::io::Error::other("first claim failed"))?;
        let first_device = first.source_device.clone();

        let second = db
            .claim_next_job_with_mode(crate::config::EngineMode::Balanced)
            .await?
            .ok_or_else(|| std::io::Error::other("second claim should pick the other device"))?;
        assert_ne!(
            first_device, second.source_device,
            "Balanced mode must not claim a second job from the same device"
        );

        // Third claim should be blocked — both devices in-flight.
        let third = db
            .claim_next_job_with_mode(crate::config::EngineMode::Balanced)
            .await?;
        assert!(
            third.is_none(),
            "Balanced mode must block when every device has an active job"
        );

        // Throughput mode ignores device grouping; it can claim the remaining job.
        let fourth = db
            .claim_next_job_with_mode(crate::config::EngineMode::Throughput)
            .await?;
        assert!(
            fourth.is_some(),
            "Throughput mode must claim regardless of in-flight devices"
        );

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    /// Regression test: a job that is archived but still stuck in an
    /// active state must not poison Balanced mode's device exclusion.
    /// Before the `AND archived = 0` fix on the device-exclusion subquery,
    /// one stale archived 'analyzing' row on a device excluded that whole
    /// device from ever being claimed from again — on the old code this
    /// assertion fails because the claim returns `None`.
    #[tokio::test]
    async fn claim_next_job_balanced_mode_ignores_archived_active_rows()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_balanced_archived_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        // A stale archived job stuck in 'analyzing' on device D.
        db.add_job(Job {
            id: 0,
            input_path: "/disk-a/stale.mkv".to_string(),
            output_path: "/disk-a/stale.out.mkv".to_string(),
            status: JobState::Analyzing,
            decision_reason: None,
            priority: 0,
            progress: 0.0,
            attempt_count: 0,
            vmaf_score: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            input_metadata_json: None,
            source_device: Some("dev:42".to_string()),
        })
        .await?;
        let stale = db
            .get_job_by_input_path("/disk-a/stale.mkv")
            .await?
            .ok_or_else(|| std::io::Error::other("missing seeded stale job"))?;
        sqlx::query("UPDATE jobs SET archived = 1 WHERE id = ?")
            .bind(stale.id)
            .execute(&db.pool)
            .await?;

        // A normal queued job on the SAME device. `add_job` is used (as in
        // `claim_next_job_balanced_mode_excludes_in_flight_devices` above)
        // so `source_device` is the same literal value as the stale job's
        // rather than whatever a real stat of a nonexistent test path
        // would resolve to.
        db.add_job(Job {
            id: 0,
            input_path: "/disk-a/ready.mkv".to_string(),
            output_path: "/disk-a/ready.out.mkv".to_string(),
            status: JobState::Queued,
            decision_reason: None,
            priority: 0,
            progress: 0.0,
            attempt_count: 0,
            vmaf_score: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            input_metadata_json: None,
            source_device: Some("dev:42".to_string()),
        })
        .await?;

        let claimed = db
            .claim_next_job_with_mode(crate::config::EngineMode::Balanced)
            .await?;
        assert!(
            claimed.is_some(),
            "an archived active-state row must not exclude its device from claiming"
        );
        let claimed = claimed.ok_or_else(|| std::io::Error::other("unreachable"))?;
        assert_eq!(claimed.input_path, "/disk-a/ready.mkv");

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    /// `update_job_status` is the one by-id path every stage of the
    /// pipeline uses to move a job into an active state (Analyzing,
    /// Encoding, Remuxing, Resuming). An archived job must never land in
    /// one of those states — it would never be claimed again, so it would
    /// sit there forever and could reproduce the Balanced-mode device
    /// exclusion deadlock. The guard must be scoped to active-state
    /// targets only: settling an archived job into a terminal status
    /// (e.g. Failed) still has to work so it can be closed out cleanly.
    #[tokio::test]
    async fn update_job_status_is_a_no_op_into_active_states_for_archived_jobs()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_update_status_archived_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        let _ = db
            .enqueue_job(
                Path::new("archived-guard.mkv"),
                Path::new("archived-guard-out.mkv"),
                SystemTime::UNIX_EPOCH,
            )
            .await?;
        let job = db
            .get_job_by_input_path("archived-guard.mkv")
            .await?
            .ok_or_else(|| std::io::Error::other("missing seeded job"))?;
        sqlx::query("UPDATE jobs SET archived = 1 WHERE id = ?")
            .bind(job.id)
            .execute(&db.pool)
            .await?;

        // Moving an archived job into an active state is a no-op: it
        // succeeds (the row exists) but the status does not change.
        // `get_job_by_input_path`/`get_job_by_id` both filter `archived =
        // 0`, so read the row back with a direct query.
        db.update_job_status(job.id, JobState::Encoding).await?;
        let after_active_attempt = sqlx::query("SELECT archived, status FROM jobs WHERE id = ?")
            .bind(job.id)
            .fetch_one(&db.pool)
            .await?;
        assert_eq!(after_active_attempt.get::<i64, _>(0), 1);
        assert_eq!(
            after_active_attempt.get::<String, _>(1),
            "queued",
            "an archived job must not be moved into an active state"
        );

        // A non-active (terminal) target status still applies normally to
        // an archived job.
        db.update_job_status(job.id, JobState::Failed).await?;
        let after_terminal = sqlx::query("SELECT archived, status FROM jobs WHERE id = ?")
            .bind(job.id)
            .fetch_one(&db.pool)
            .await?;
        assert_eq!(after_terminal.get::<i64, _>(0), 1);
        assert_eq!(after_terminal.get::<String, _>(1), "failed");

        // A genuinely missing id must still be a real error, not a silent
        // no-op.
        let missing_id = job.id + 1_000_000;
        let missing_result = db.update_job_status(missing_id, JobState::Encoding).await;
        assert!(
            missing_result.is_err(),
            "a nonexistent job id must still error, not no-op"
        );

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

        // A job that is both archived AND stuck in an active state (e.g. an
        // unclean shutdown that raced an archive, or data left over from
        // before this fix). Nothing can be running it after a restart, so
        // it must be reset too — otherwise it sits in 'encoding' forever
        // and, in Balanced mode, keeps excluding its whole device from
        // claiming anything. It stays archived: `archived = 0` filters
        // elsewhere still keep it out of every claim/analysis query.
        let _ = db
            .enqueue_job(
                Path::new("archived-encoding.mkv"),
                Path::new("archived-encoding-out.mkv"),
                SystemTime::UNIX_EPOCH,
            )
            .await?;
        let archived_job = db
            .get_job_by_input_path("archived-encoding.mkv")
            .await?
            .ok_or_else(|| std::io::Error::other("missing seeded archived job"))?;
        db.update_job_status(archived_job.id, JobState::Encoding)
            .await?;
        sqlx::query("UPDATE jobs SET archived = 1 WHERE id = ?")
            .bind(archived_job.id)
            .execute(&db.pool)
            .await?;

        let reset = db.reset_interrupted_jobs().await?;
        assert_eq!(reset, 4);

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

        // `get_job_by_input_path`/`get_job_by_id` both filter `archived =
        // 0`, so read the row back with a direct query.
        let archived_after = sqlx::query("SELECT archived, status FROM jobs WHERE id = ?")
            .bind(archived_job.id)
            .fetch_one(&db.pool)
            .await?;
        assert_eq!(
            archived_after.get::<String, _>(1),
            "queued",
            "archived jobs stuck in an active state must be reset too"
        );
        assert_eq!(
            archived_after.get::<i64, _>(0),
            1,
            "resetting status must not un-archive the job"
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

    #[tokio::test]
    async fn get_jobs_filtered_search_matches_paths_decisions_and_failures()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_job_search_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        db.enqueue_job(
            Path::new("/media/plain.mkv"),
            Path::new("/output/plain.mkv"),
            SystemTime::UNIX_EPOCH,
        )
        .await?;
        let decision_job = db
            .get_job_by_input_path("/media/plain.mkv")
            .await?
            .ok_or_else(|| std::io::Error::other("missing decision job"))?;
        db.add_decision(
            decision_job.id,
            "skip",
            "hdr_metadata|summary=HDR metadata should stay visible",
        )
        .await?;

        db.enqueue_job(
            Path::new("/media/failure.mkv"),
            Path::new("/output/failure.mkv"),
            SystemTime::UNIX_EPOCH,
        )
        .await?;
        let failure_job = db
            .get_job_by_input_path("/media/failure.mkv")
            .await?
            .ok_or_else(|| std::io::Error::other("missing failure job"))?;
        db.upsert_job_failure_explanation(
            failure_job.id,
            &failure_from_summary("Subtitle burn failed during FFmpeg execution"),
        )
        .await?;

        db.enqueue_job(
            Path::new("/media/path-needle.mkv"),
            Path::new("/output/path-needle.mkv"),
            SystemTime::UNIX_EPOCH,
        )
        .await?;
        let path_job = db
            .get_job_by_input_path("/media/path-needle.mkv")
            .await?
            .ok_or_else(|| std::io::Error::other("missing path job"))?;

        let query = |search: &str| JobFilterQuery {
            limit: 50,
            offset: 0,
            search: Some(search.to_string()),
            archived: Some(false),
            ..Default::default()
        };

        let hdr_matches = db.get_jobs_filtered(query("HDR metadata")).await?;
        assert_eq!(hdr_matches.len(), 1);
        assert_eq!(hdr_matches[0].id, decision_job.id);

        let subtitle_matches = db.get_jobs_filtered(query("subtitle burn")).await?;
        assert_eq!(subtitle_matches.len(), 1);
        assert_eq!(subtitle_matches[0].id, failure_job.id);

        let path_matches = db.get_jobs_filtered(query("path-needle")).await?;
        assert_eq!(path_matches.len(), 1);
        assert_eq!(path_matches[0].id, path_job.id);

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn test_reanalyze_jobs_under_path() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_reanalyze_path_{}.db", token));
        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        // Setup jobs
        db.enqueue_job(
            Path::new("/root/sub/job1.mkv"),
            Path::new("/root/sub/job1.mp4"),
            SystemTime::now(),
        )
        .await?;
        let job1 = db
            .get_job_by_input_path("/root/sub/job1.mkv")
            .await?
            .ok_or("job1 not found")?;
        db.update_job_status(job1.id, JobState::Completed).await?;

        db.enqueue_job(
            Path::new("/root/sub/job2.mkv"),
            Path::new("/root/sub/job2.mp4"),
            SystemTime::now(),
        )
        .await?;
        let job2 = db
            .get_job_by_input_path("/root/sub/job2.mkv")
            .await?
            .ok_or("job2 not found")?;
        db.update_job_status(job2.id, JobState::Encoding).await?;

        db.enqueue_job(
            Path::new("/root/other/job3.mkv"),
            Path::new("/root/other/job3.mp4"),
            SystemTime::now(),
        )
        .await?;
        let job3 = db
            .get_job_by_input_path("/root/other/job3.mkv")
            .await?
            .ok_or("job3 not found")?;
        db.update_job_status(job3.id, JobState::Completed).await?;

        // Add some decisions/stats
        sqlx::query("INSERT INTO decisions (job_id, action, reason) VALUES (?, 'skip', 'test')")
            .bind(job1.id)
            .execute(&db.pool)
            .await?;
        sqlx::query(
            "INSERT INTO encode_stats (job_id, input_size_bytes, output_size_bytes, compression_ratio, encode_time_seconds, encode_speed, avg_bitrate_kbps) VALUES (?, 100, 50, 0.5, 10.0, 1.0, 1000.0)",
        )
        .bind(job1.id)
        .execute(&db.pool)
        .await?;

        // Reanalyze /root/sub
        let count = db.reanalyze_jobs_under_path("/root/sub").await?;
        assert_eq!(count, 1); // Only job1, job2 is active

        let job1_after = db
            .get_job_by_id(job1.id)
            .await?
            .ok_or("job1_after not found")?;
        let job2_after = db
            .get_job_by_id(job2.id)
            .await?
            .ok_or("job2_after not found")?;
        let job3_after = db
            .get_job_by_id(job3.id)
            .await?
            .ok_or("job3_after not found")?;

        assert_eq!(job1_after.status, JobState::Queued);
        assert_eq!(job2_after.status, JobState::Encoding);
        assert_eq!(job3_after.status, JobState::Completed);

        // Verify data cleared for job1
        let decisions = sqlx::query("SELECT COUNT(*) FROM decisions WHERE job_id = ?")
            .bind(job1.id)
            .fetch_one(&db.pool)
            .await?
            .get::<i64, _>(0);
        assert_eq!(decisions, 0);

        let stats = sqlx::query("SELECT COUNT(*) FROM encode_stats WHERE job_id = ?")
            .bind(job1.id)
            .fetch_one(&db.pool)
            .await?
            .get::<i64, _>(0);
        assert_eq!(stats, 0);

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn purge_jobs_by_filter_empty_status_does_not_delete()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_purge_empty_status_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;
        db.enqueue_job(
            Path::new("/tmp/purge_a.mkv"),
            Path::new("/tmp/purge_a.out.mkv"),
            SystemTime::UNIX_EPOCH,
        )
        .await?;
        db.enqueue_job(
            Path::new("/tmp/purge_b.mkv"),
            Path::new("/tmp/purge_b.out.mkv"),
            SystemTime::UNIX_EPOCH,
        )
        .await?;

        // Caller passed `Some(status filter)` but nothing parsed — must NOT
        // fall through to "delete every row".
        let removed = db.purge_jobs_by_filter(Some(Vec::new()), None).await?;
        assert_eq!(removed, 0);

        let remaining: i64 = sqlx::query("SELECT COUNT(*) FROM jobs")
            .fetch_one(&db.pool)
            .await?
            .get::<i64, _>(0);
        assert_eq!(remaining, 2);

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn test_batch_mutation_safety_predicates()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_batch_mutation_safety_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        // 1. Create a job and mark it encoding (active)
        db.enqueue_job(
            Path::new("/tmp/active_a.mkv"),
            Path::new("/tmp/active_a.out.mkv"),
            SystemTime::UNIX_EPOCH,
        )
        .await?;
        let active_job = db
            .get_job_by_input_path("/tmp/active_a.mkv")
            .await?
            .ok_or("active job not found")?;
        db.update_job_status(active_job.id, JobState::Encoding)
            .await?;

        // 2. Call batch_restart_jobs and batch_delete_jobs on the active job
        let restarted = db.batch_restart_jobs(&[active_job.id]).await?;
        assert_eq!(restarted, 0);

        let deleted = db.batch_delete_jobs(&[active_job.id]).await?;
        assert_eq!(deleted, 0);

        // 3. Create an archived job and assert restart cannot unarchive/requeue it
        db.enqueue_job(
            Path::new("/tmp/archived_b.mkv"),
            Path::new("/tmp/archived_b.out.mkv"),
            SystemTime::UNIX_EPOCH,
        )
        .await?;
        let archived_job = db
            .get_job_by_input_path("/tmp/archived_b.mkv")
            .await?
            .ok_or("archived job not found")?;
        db.update_job_status(archived_job.id, JobState::Failed)
            .await?;
        // Archive it directly
        db.delete_job(archived_job.id).await?;

        // Assert it is indeed archived
        let archived_job_check = sqlx::query("SELECT archived, status FROM jobs WHERE id = ?")
            .bind(archived_job.id)
            .fetch_one(&db.pool)
            .await?;
        assert_eq!(archived_job_check.get::<i64, _>(0), 1);

        // Call batch_restart_jobs on the archived job
        let restarted_archived = db.batch_restart_jobs(&[archived_job.id]).await?;
        assert_eq!(restarted_archived, 0);

        // Assert it is still archived
        let archived_job_check_after =
            sqlx::query("SELECT archived, status FROM jobs WHERE id = ?")
                .bind(archived_job.id)
                .fetch_one(&db.pool)
                .await?;
        assert_eq!(archived_job_check_after.get::<i64, _>(0), 1);
        assert_ne!(archived_job_check_after.get::<String, _>(1), "queued");

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    /// Id lists longer than one `IN (...)` chunk must still apply in full.
    /// Before chunking, a list past SQLite's `SQLITE_MAX_VARIABLE_NUMBER`
    /// (32766) failed the whole statement with "too many SQL variables" — which
    /// a library-wide reanalyze reaches in ordinary use. This uses a list a few
    /// chunks long, which is enough to prove every chunk is executed.
    #[tokio::test]
    async fn batch_updates_apply_across_id_chunks()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_id_chunk_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        let total = ID_CHUNK * 2 + 7;
        let mut ids = Vec::with_capacity(total);
        for i in 0..total {
            db.enqueue_job(
                Path::new(&format!("/media/chunk-{i}.mkv")),
                Path::new(&format!("/media/chunk-{i}.out.mkv")),
                SystemTime::UNIX_EPOCH,
            )
            .await?;
            let job = db
                .get_job_by_input_path(&format!("/media/chunk-{i}.mkv"))
                .await?
                .ok_or_else(|| std::io::Error::other("missing enqueued chunk job"))?;
            ids.push(job.id);
        }

        // Reads chunk too: every id must come back, not just the first chunk.
        let fetched = db.get_jobs_by_ids(&ids).await?;
        assert_eq!(fetched.len(), total);

        let cancelled = db.batch_cancel_jobs(&ids).await?;
        assert_eq!(cancelled, total as u64);

        let archived = db.batch_delete_jobs(&ids).await?;
        assert_eq!(archived, total as u64);

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    /// A restart clears the job's stale failure explanation. Without this, a
    /// requeued job kept rendering the previous run's failure banner while it
    /// sat healthy in the queue.
    #[tokio::test]
    async fn restart_and_reanalyze_clear_stale_failure_explanations()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut db_path = std::env::temp_dir();
        let token: u64 = rand::random();
        db_path.push(format!("alchemist_stale_failure_test_{}.db", token));

        let db = Db::new(db_path.to_string_lossy().as_ref()).await?;

        let restarted_path = "/media/restart-me.mkv";
        let reanalyzed_path = "/media/reanalyze-me.mkv";
        for path in [restarted_path, reanalyzed_path] {
            db.enqueue_job(
                Path::new(path),
                Path::new(&format!("{path}.out")),
                SystemTime::UNIX_EPOCH,
            )
            .await?;
        }

        let restarted = db
            .get_job_by_input_path(restarted_path)
            .await?
            .ok_or_else(|| std::io::Error::other("missing restart job"))?;
        let reanalyzed = db
            .get_job_by_input_path(reanalyzed_path)
            .await?
            .ok_or_else(|| std::io::Error::other("missing reanalyze job"))?;

        for id in [restarted.id, reanalyzed.id] {
            db.update_job_status(id, JobState::Failed).await?;
            db.upsert_job_failure_explanation(id, &failure_from_summary("Transcode failed: boom"))
                .await?;
        }
        let before = db
            .get_job_failure_explanations(&[restarted.id, reanalyzed.id])
            .await?;
        assert_eq!(before.len(), 2);

        db.batch_restart_jobs(&[restarted.id]).await?;
        db.batch_reanalyze_jobs(&[reanalyzed.id]).await?;

        let after = db
            .get_job_failure_explanations(&[restarted.id, reanalyzed.id])
            .await?;
        assert!(
            after.is_empty(),
            "restart/reanalyze left stale failure explanations: {after:?}"
        );

        drop(db);
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
