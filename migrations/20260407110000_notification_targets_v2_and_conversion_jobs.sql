CREATE TABLE IF NOT EXISTS notification_targets_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    target_type TEXT CHECK(target_type IN ('discord_webhook', 'discord_bot', 'gotify', 'webhook', 'telegram', 'email')) NOT NULL,
    config_json TEXT NOT NULL DEFAULT '{}',
    events TEXT NOT NULL DEFAULT '["encode.failed","encode.completed"]',
    enabled BOOLEAN DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO notification_targets_new (id, name, target_type, config_json, events, enabled, created_at)
SELECT
    id,
    name,
    CASE target_type
        WHEN 'discord' THEN 'discord_webhook'
        WHEN 'gotify' THEN 'gotify'
        ELSE 'webhook'
    END,
    CASE target_type
        WHEN 'discord' THEN json_object('webhook_url', endpoint_url)
        WHEN 'gotify' THEN json_object('server_url', endpoint_url, 'app_token', COALESCE(auth_token, ''))
        ELSE json_object('url', endpoint_url, 'auth_token', auth_token)
    END,
    COALESCE(events, '["failed","completed"]'),
    enabled,
    created_at
FROM notification_targets;

DROP TABLE notification_targets;
ALTER TABLE notification_targets_new RENAME TO notification_targets;

CREATE INDEX IF NOT EXISTS idx_notification_targets_enabled
    ON notification_targets(enabled);

CREATE TABLE IF NOT EXISTS conversion_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    upload_path TEXT NOT NULL,
    output_path TEXT,
    mode TEXT NOT NULL,
    settings_json TEXT NOT NULL,
    probe_json TEXT,
    linked_job_id INTEGER REFERENCES jobs(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'uploaded',
    expires_at TEXT NOT NULL,
    downloaded_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_conversion_jobs_status_updated_at
    ON conversion_jobs(status, updated_at);

CREATE INDEX IF NOT EXISTS idx_conversion_jobs_expires_at
    ON conversion_jobs(expires_at);

CREATE INDEX IF NOT EXISTS idx_conversion_jobs_linked_job_id
    ON conversion_jobs(linked_job_id);

INSERT OR REPLACE INTO schema_info (key, value) VALUES
    ('schema_version', '7'),
    ('min_compatible_version', '0.2.5'),
    ('last_updated', datetime('now'));
