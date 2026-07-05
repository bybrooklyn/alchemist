-- Per-job log retrieval filters logs by job_id. Without an index this scans
-- the entire logs table for every job detail view. Additive index only.

CREATE INDEX IF NOT EXISTS idx_logs_job_id ON logs(job_id);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '18'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
