# Database Schema

Internal database structure and migration policy for Alchemist.

Alchemist uses SQLite for persistence. The database file is located at `data/alchemist.db`.

## Tables

The database is composed of several tables to track jobs, statistics, and user sessions.

### `jobs`
| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `input_path` | TEXT | Source file path (unique) |
| `output_path` | TEXT | Destination file path |
| `status` | TEXT | Job state (queued/encoding/completed/failed/etc) |
| `mtime_hash` | TEXT | File modification time hash |
| `priority` | INTEGER | Job priority (higher = first) |
| `progress` | REAL | Encoding progress 0-100 |
| `attempt_count` | INTEGER | Retry count |
| `created_at` | DATETIME | Job creation time |
| `updated_at` | DATETIME | Last status update |

### `encode_stats`
| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `job_id` | INTEGER | Foreign key to jobs |
| `input_size_bytes` | INTEGER | Original file size |
| `output_size_bytes` | INTEGER | Encoded file size |
| `compression_ratio` | REAL | input/output ratio |
| `encode_time_seconds` | REAL | Total encoding time |
| `encode_speed` | REAL | Frames per second |
| `avg_bitrate_kbps` | REAL | Output bitrate |
| `vmaf_score` | REAL | Quality score (0-100) |
| `created_at` | DATETIME | Completion time |

### `decisions`
| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `job_id` | INTEGER | Foreign key to jobs |
| `action` | TEXT | Action taken (encode/skip/revert) |
| `reason` | TEXT | Human-readable explanation |
| `created_at` | DATETIME | Decision time |

### `users`
| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `username` | TEXT | Unique username |
| `password_hash` | TEXT | Argon2 password hash |
| `created_at` | DATETIME | Account creation time |

### `sessions`
| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `user_id` | INTEGER | Foreign key to users |
| `token` | TEXT | Session token (unique) |
| `created_at` | DATETIME | Session start |
| `expires_at` | DATETIME | Session expiration |

## Database Migration Policy

> **Baseline Version: 0.2.5**

All database migrations maintain **backwards compatibility** with the v0.2.5 schema. This means newer app versions will work with older database files, and no data is lost during upgrades.

### Migration Rules

To ensure stability, we follow strict rules for modifying the database schema:

#### Allowed Operations
- Add new tables with `CREATE TABLE IF NOT EXISTS`
- Add new columns with `NULL` or `DEFAULT` values
- Add new indexes with `CREATE INDEX IF NOT EXISTS`
- Insert new configuration rows

#### Forbidden Operations
- Never remove columns
- Never rename columns
- Never change column types
- Never remove tables
- Never add `NOT NULL` columns without defaults

### Schema Version Tracking

The `schema_info` table tracks compatibility:

```sql
SELECT value FROM schema_info WHERE key = 'min_compatible_version';
-- Returns: "0.2.5"
```
