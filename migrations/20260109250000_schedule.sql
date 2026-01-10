CREATE TABLE IF NOT EXISTS schedule_windows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    days_of_week TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT 1
);
