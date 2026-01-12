-- Stability and performance indexes (v0.2.5+ compatible)

CREATE INDEX IF NOT EXISTS idx_jobs_status_priority_created_at
    ON jobs(status, priority DESC, created_at);

CREATE INDEX IF NOT EXISTS idx_jobs_status_updated_at
    ON jobs(status, updated_at);

CREATE INDEX IF NOT EXISTS idx_jobs_updated_at
    ON jobs(updated_at);

CREATE INDEX IF NOT EXISTS idx_logs_created_at
    ON logs(created_at);

CREATE INDEX IF NOT EXISTS idx_decisions_job_id_created_at
    ON decisions(job_id, created_at);

CREATE INDEX IF NOT EXISTS idx_encode_stats_created_at
    ON encode_stats(created_at);

CREATE INDEX IF NOT EXISTS idx_schedule_windows_enabled
    ON schedule_windows(enabled);

CREATE INDEX IF NOT EXISTS idx_notification_targets_enabled
    ON notification_targets(enabled);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '2'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
