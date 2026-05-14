-- PERF-2: Track the source device per job so the scheduler can avoid
-- making two HDDs seek against themselves. Nullable for legacy rows
-- and for paths where device resolution fails (network mounts, etc.).
ALTER TABLE jobs ADD COLUMN source_device TEXT;

-- Partial index used by the Balanced-mode claim query to find which
-- devices currently have a running job, so newly claimed work can be
-- excluded from the same device until the running one finishes.
CREATE INDEX IF NOT EXISTS idx_jobs_source_device_active
    ON jobs(source_device)
    WHERE status IN ('queued', 'analyzing', 'encoding', 'remuxing', 'resuming');

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '14'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
