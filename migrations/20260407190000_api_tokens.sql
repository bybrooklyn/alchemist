CREATE TABLE IF NOT EXISTS api_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    access_level TEXT CHECK(access_level IN ('read_only', 'full_access')) NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_used_at DATETIME,
    revoked_at DATETIME
);

CREATE INDEX IF NOT EXISTS idx_api_tokens_active
    ON api_tokens(revoked_at, access_level);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '8'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
