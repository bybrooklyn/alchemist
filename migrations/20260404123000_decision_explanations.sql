ALTER TABLE decisions ADD COLUMN reason_code TEXT;
ALTER TABLE decisions ADD COLUMN reason_payload_json TEXT;

CREATE TABLE IF NOT EXISTS job_failure_explanations (
    job_id INTEGER PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
    legacy_summary TEXT,
    code TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_decisions_reason_code
    ON decisions(reason_code);

CREATE INDEX IF NOT EXISTS idx_job_failure_explanations_code
    ON job_failure_explanations(code);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '6'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
