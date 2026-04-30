CREATE TABLE IF NOT EXISTS hardware_detection_cache (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    cache_key TEXT NOT NULL,
    fingerprint_json TEXT NOT NULL,
    hardware_info_json TEXT NOT NULL,
    probe_log_json TEXT NOT NULL,
    detected_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_hardware_detection_cache_key
    ON hardware_detection_cache(cache_key);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '13'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
