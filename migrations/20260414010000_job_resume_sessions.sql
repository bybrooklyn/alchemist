CREATE TABLE IF NOT EXISTS job_resume_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL UNIQUE REFERENCES jobs(id) ON DELETE CASCADE,
    strategy TEXT NOT NULL,
    plan_hash TEXT NOT NULL,
    mtime_hash TEXT NOT NULL,
    temp_dir TEXT NOT NULL,
    concat_manifest_path TEXT NOT NULL,
    segment_length_secs INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS job_resume_segments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    segment_index INTEGER NOT NULL,
    start_secs REAL NOT NULL,
    duration_secs REAL NOT NULL,
    temp_path TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    attempt_count INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(job_id, segment_index)
);

CREATE INDEX IF NOT EXISTS idx_job_resume_sessions_status
    ON job_resume_sessions(status);

CREATE INDEX IF NOT EXISTS idx_job_resume_segments_job_status
    ON job_resume_segments(job_id, status);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '9'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
