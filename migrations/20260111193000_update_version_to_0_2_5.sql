-- Migration to update minimum compatible version to 0.2.5
INSERT OR REPLACE INTO schema_info (key, value) VALUES 
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
