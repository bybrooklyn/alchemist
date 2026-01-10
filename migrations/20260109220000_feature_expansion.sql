-- Feature Expansion Tables

-- Logs table
CREATE TABLE IF NOT EXISTS logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    level TEXT NOT NULL, -- 'info', 'warn', 'error', 'debug'
    job_id INTEGER,
    message TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Watch directories
CREATE TABLE IF NOT EXISTS watch_dirs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    enabled INTEGER DEFAULT 1,
    recursive INTEGER DEFAULT 1,
    extensions TEXT, -- JSON array or NULL
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Notification settings  
CREATE TABLE IF NOT EXISTS notification_settings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider TEXT NOT NULL, -- 'gotify', 'discord', 'webhook'
    config TEXT NOT NULL, -- JSON config
    on_complete INTEGER DEFAULT 0,
    on_fail INTEGER DEFAULT 1,
    on_daily_summary INTEGER DEFAULT 0,
    enabled INTEGER DEFAULT 1,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Schedule settings
CREATE TABLE IF NOT EXISTS schedule_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
