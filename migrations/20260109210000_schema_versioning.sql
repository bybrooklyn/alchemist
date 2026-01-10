-- Schema versioning table to track database compatibility
-- This establishes v0.2.4 as the baseline for forward compatibility

CREATE TABLE IF NOT EXISTS schema_info (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

-- Insert baseline version info
INSERT OR REPLACE INTO schema_info (key, value) VALUES 
    ('schema_version', '1'),
    ('min_compatible_version', '0.2.4'),
    ('created_at', datetime('now'));
