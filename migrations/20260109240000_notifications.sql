CREATE TABLE IF NOT EXISTS notification_targets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    target_type TEXT CHECK(target_type IN ('gotify', 'discord', 'webhook')) NOT NULL,
    endpoint_url TEXT NOT NULL,
    auth_token TEXT,
    events TEXT DEFAULT '["failed","completed"]',
    enabled BOOLEAN DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
