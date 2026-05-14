-- Stats top-reason trends (added in 0.3.2-rc.2 / OBS-3) were doing full
-- table scans on decisions and job_failure_explanations every time the
-- Stats page asked for trends=true. On a large library the page could
-- look indefinitely hung. These indexes back the two trend queries in
-- src/db/stats.rs::get_skip_reason_trend and ::get_failure_code_trend.

CREATE INDEX IF NOT EXISTS idx_decisions_created_at_action
    ON decisions(created_at, action);

CREATE INDEX IF NOT EXISTS idx_failure_explanations_updated_at
    ON job_failure_explanations(updated_at);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '16'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
