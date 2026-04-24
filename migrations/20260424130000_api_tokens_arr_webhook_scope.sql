ALTER TABLE api_tokens ADD COLUMN access_scope TEXT;

CREATE INDEX IF NOT EXISTS idx_api_tokens_active_scope
    ON api_tokens(revoked_at, access_scope, access_level);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '11'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
