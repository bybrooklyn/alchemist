use crate::error::Result;
use sqlx::Row;

use super::timed_query;
use super::types::*;
use super::Db;

impl Db {
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

    /// Record a single encode attempt outcome
    pub async fn insert_encode_attempt(&self, input: EncodeAttemptInput) -> Result<()> {
        sqlx::query(
            "INSERT INTO encode_attempts
             (job_id, attempt_number, started_at, finished_at, outcome,
              failure_code, failure_summary, input_size_bytes, output_size_bytes,
              encode_time_seconds)
             VALUES (?, ?, ?, datetime('now'), ?, ?, ?, ?, ?, ?)",
        )
        .bind(input.job_id)
        .bind(input.attempt_number)
        .bind(input.started_at)
        .bind(input.outcome)
        .bind(input.failure_code)
        .bind(input.failure_summary)
        .bind(input.input_size_bytes)
        .bind(input.output_size_bytes)
        .bind(input.encode_time_seconds)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all encode attempts for a job, ordered by attempt_number
    pub async fn get_encode_attempts_by_job(&self, job_id: i64) -> Result<Vec<EncodeAttempt>> {
        let attempts = sqlx::query_as::<_, EncodeAttempt>(
            "SELECT id, job_id, attempt_number, started_at, finished_at, outcome,
                    failure_code, failure_summary, input_size_bytes, output_size_bytes,
                    encode_time_seconds, created_at
             FROM encode_attempts
             WHERE job_id = ?
             ORDER BY attempt_number ASC",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(attempts)
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

    pub async fn get_aggregated_stats(&self) -> Result<AggregatedStats> {
        let pool = &self.pool;
        timed_query("get_aggregated_stats", || async {
            let row = sqlx::query(
                "SELECT
                    (SELECT COUNT(*) FROM jobs WHERE archived = 0) as total_jobs,
                    (SELECT COUNT(*) FROM jobs WHERE status = 'completed' AND archived = 0) as completed_jobs,
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
                    COALESCE(SUM(e.input_size_bytes - e.output_size_bytes), 0) as bytes_saved,
                    COUNT(*) as job_count
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
                job_count: row.get("job_count"),
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

    pub async fn get_job_stats(&self) -> Result<JobStats> {
        let pool = &self.pool;
        timed_query("get_job_stats", || async {
            let rows = sqlx::query(
                "SELECT status, COUNT(*) as count FROM jobs WHERE archived = 0 GROUP BY status",
            )
            .fetch_all(pool)
            .await?;

            let mut stats = JobStats::default();
            for row in rows {
                let status_str: String = row.get("status");
                let count: i64 = row.get("count");

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

    pub async fn get_daily_summary_stats(&self) -> Result<DailySummaryStats> {
        let pool = &self.pool;
        timed_query("get_daily_summary_stats", || async {
            let row = sqlx::query(
                "SELECT
                    COALESCE(SUM(CASE WHEN status = 'completed' AND DATE(updated_at, 'localtime') = DATE('now', 'localtime') THEN 1 ELSE 0 END), 0) AS completed,
                    COALESCE(SUM(CASE WHEN status = 'failed' AND DATE(updated_at, 'localtime') = DATE('now', 'localtime') THEN 1 ELSE 0 END), 0) AS failed,
                    COALESCE(SUM(CASE WHEN status = 'skipped' AND DATE(updated_at, 'localtime') = DATE('now', 'localtime') THEN 1 ELSE 0 END), 0) AS skipped
                 FROM jobs",
            )
            .fetch_one(pool)
            .await?;

            let completed: i64 = row.get("completed");
            let failed: i64 = row.get("failed");
            let skipped: i64 = row.get("skipped");

            let bytes_row = sqlx::query(
                "SELECT COALESCE(SUM(input_size_bytes - output_size_bytes), 0) AS bytes_saved
                 FROM encode_stats
                 WHERE DATE(created_at, 'localtime') = DATE('now', 'localtime')",
            )
            .fetch_one(pool)
            .await?;
            let bytes_saved: i64 = bytes_row.get("bytes_saved");

            let failure_rows = sqlx::query(
                "SELECT code, COUNT(*) AS count
                 FROM job_failure_explanations
                 WHERE DATE(updated_at, 'localtime') = DATE('now', 'localtime')
                 GROUP BY code
                 ORDER BY count DESC, code ASC
                 LIMIT 3",
            )
            .fetch_all(pool)
            .await?;
            let top_failure_reasons = failure_rows
                .into_iter()
                .map(|row| row.get::<String, _>("code"))
                .collect::<Vec<_>>();

            let skip_rows = sqlx::query(
                "SELECT COALESCE(reason_code, action) AS code, COUNT(*) AS count
                 FROM decisions
                 WHERE action = 'skip'
                   AND DATE(created_at, 'localtime') = DATE('now', 'localtime')
                 GROUP BY COALESCE(reason_code, action)
                 ORDER BY count DESC, code ASC
                 LIMIT 3",
            )
            .fetch_all(pool)
            .await?;
            let top_skip_reasons = skip_rows
                .into_iter()
                .map(|row| row.get::<String, _>("code"))
                .collect::<Vec<_>>();

            Ok(DailySummaryStats {
                completed,
                failed,
                skipped,
                bytes_saved,
                top_failure_reasons,
                top_skip_reasons,
            })
        })
        .await
    }

    pub async fn get_skip_reason_counts(&self) -> Result<Vec<(String, i64)>> {
        let pool = &self.pool;
        timed_query("get_skip_reason_counts", || async {
            let rows = sqlx::query(
                "SELECT COALESCE(reason_code, action) AS code, COUNT(*) AS count
                 FROM decisions
                 WHERE action = 'skip'
                   AND DATE(created_at, 'localtime') = DATE('now', 'localtime')
                 GROUP BY COALESCE(reason_code, action)
                 ORDER BY count DESC, code ASC
                 LIMIT 20",
            )
            .fetch_all(pool)
            .await?;
            Ok(rows
                .into_iter()
                .map(|row| {
                    let code: String = row.get("code");
                    let count: i64 = row.get("count");
                    (code, count)
                })
                .collect())
        })
        .await
    }
}
