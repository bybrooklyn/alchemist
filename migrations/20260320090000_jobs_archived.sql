ALTER TABLE jobs ADD COLUMN archived BOOLEAN NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_jobs_archived_status_updated_at
    ON jobs(archived, status, updated_at);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '3'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
