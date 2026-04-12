CREATE TABLE IF NOT EXISTS encode_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL,
    started_at TEXT,
    finished_at TEXT NOT NULL DEFAULT (datetime('now')),
    outcome TEXT NOT NULL CHECK(outcome IN ('completed', 'failed', 'cancelled')),
    failure_code TEXT,
    failure_summary TEXT,
    input_size_bytes INTEGER,
    output_size_bytes INTEGER,
    encode_time_seconds REAL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_encode_attempts_job_id ON encode_attempts(job_id);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '8'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
