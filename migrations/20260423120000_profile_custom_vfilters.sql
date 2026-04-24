-- Add custom_vfilters column to library_profiles
ALTER TABLE library_profiles ADD COLUMN custom_vfilters TEXT;

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '10'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
