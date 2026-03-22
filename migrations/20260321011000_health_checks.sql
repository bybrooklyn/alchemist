ALTER TABLE jobs ADD COLUMN health_issues TEXT;
ALTER TABLE jobs ADD COLUMN last_health_check TEXT;

CREATE TABLE IF NOT EXISTS health_scan_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    files_checked INTEGER NOT NULL DEFAULT 0,
    issues_found INTEGER NOT NULL DEFAULT 0
);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '5'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
