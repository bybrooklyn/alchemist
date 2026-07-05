-- Scan-collision checks look up jobs by output_path. Without an index this
-- degrades to a full table scan per candidate, making scans quadratic on
-- large libraries. Additive index only.

CREATE INDEX IF NOT EXISTS idx_jobs_output_path ON jobs(output_path);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '17'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
