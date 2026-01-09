-- Create Jobs Table
CREATE TABLE IF NOT EXISTS jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    input_path TEXT NOT NULL UNIQUE,
    output_path TEXT NOT NULL,
    status TEXT NOT NULL,
    mtime_hash TEXT NOT NULL,
    priority INTEGER DEFAULT 0,
    progress REAL DEFAULT 0.0,
    attempt_count INTEGER DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create Decisions Table
CREATE TABLE IF NOT EXISTS decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL,
    action TEXT NOT NULL,
    reason TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
);

-- Create Encode Stats Table
CREATE TABLE IF NOT EXISTS encode_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL UNIQUE,
    input_size_bytes INTEGER NOT NULL,
    output_size_bytes INTEGER NOT NULL,
    compression_ratio REAL NOT NULL,
    encode_time_seconds REAL NOT NULL,
    encode_speed REAL NOT NULL,
    avg_bitrate_kbps REAL NOT NULL,
    vmaf_score REAL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE
);
