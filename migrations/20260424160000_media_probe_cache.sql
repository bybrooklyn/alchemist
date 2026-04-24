CREATE TABLE IF NOT EXISTS media_probe_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    input_path TEXT NOT NULL,
    mtime_ns INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL,
    probe_version TEXT NOT NULL,
    analysis_json TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_accessed_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(input_path, mtime_ns, size_bytes, probe_version)
);

CREATE INDEX IF NOT EXISTS idx_media_probe_cache_last_accessed
    ON media_probe_cache(last_accessed_at);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '12'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
