-- PERF-3: Incremental scan support.
--
-- 1. Track when each watch directory was last fully scanned so optional
--    aggressive directory pruning can skip subtrees whose mtime hasn't
--    advanced. Stored as unix epoch seconds; NULL on first scan.
ALTER TABLE watch_dirs ADD COLUMN last_scanned_at INTEGER;

-- 2. Add an optional file identity hint to the probe cache. Inode on Unix
--    (MetadataExt::ino()) or volume file index on Windows when cheaply
--    available. Lets the scanner skip expensive analysis when path, size,
--    mtime, *and* (when present on both sides) file_id all match.
ALTER TABLE media_probe_cache ADD COLUMN file_id TEXT;

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '15'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
