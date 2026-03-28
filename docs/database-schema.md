---
title: Database Schema
description: SQLite schema reference and migration policy.
---

Database location:

- Linux/macOS: `~/.config/alchemist/alchemist.db`
- Linux/macOS with XDG: `$XDG_CONFIG_HOME/alchemist/alchemist.db`
- Windows: `%APPDATA%\Alchemist\alchemist.db`
- Override: `ALCHEMIST_DB_PATH`

## `jobs`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `input_path` | TEXT | Unique source path |
| `output_path` | TEXT | Planned output path |
| `status` | TEXT | Current job state |
| `mtime_hash` | TEXT | File modification fingerprint |
| `priority` | INTEGER | Queue priority |
| `progress` | REAL | Progress percentage |
| `attempt_count` | INTEGER | Retry count |
| `created_at` | DATETIME | Creation timestamp |
| `updated_at` | DATETIME | Last update timestamp |
| `archived` | BOOLEAN | Archived flag for cleared completed jobs |
| `health_issues` | TEXT | Serialized health issues from Library Doctor |
| `last_health_check` | TEXT | Last library health check timestamp |

## `encode_stats`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `job_id` | INTEGER | Unique foreign key to `jobs.id` |
| `input_size_bytes` | INTEGER | Original size |
| `output_size_bytes` | INTEGER | Output size |
| `compression_ratio` | REAL | Compression ratio |
| `encode_time_seconds` | REAL | Total encode duration |
| `encode_speed` | REAL | Reported encode speed |
| `avg_bitrate_kbps` | REAL | Average output bitrate |
| `vmaf_score` | REAL | Optional VMAF score |
| `created_at` | DATETIME | Insert timestamp |
| `output_codec` | TEXT | Output codec recorded with the stats row |

## `decisions`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `job_id` | INTEGER | Foreign key to `jobs.id` |
| `action` | TEXT | Planner or post-encode action |
| `reason` | TEXT | Machine-readable reason string |
| `created_at` | DATETIME | Insert timestamp |

## `users`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `username` | TEXT | Unique login name |
| `password_hash` | TEXT | Argon2 password hash |
| `created_at` | DATETIME | Insert timestamp |

## `sessions`

| Column | Type | Description |
|--------|------|-------------|
| `token` | TEXT | Primary key session token |
| `user_id` | INTEGER | Foreign key to `users.id` |
| `expires_at` | DATETIME | Expiration timestamp |
| `created_at` | DATETIME | Insert timestamp |

## `logs`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `level` | TEXT | Log level |
| `job_id` | INTEGER | Optional job association |
| `message` | TEXT | Log message |
| `created_at` | DATETIME | Insert timestamp |

## `ui_preferences`

| Column | Type | Description |
|--------|------|-------------|
| `key` | TEXT | Primary key |
| `value` | TEXT | Stored preference value |
| `updated_at` | DATETIME | Last update timestamp |

## `watch_dirs`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `path` | TEXT | Unique watched path |
| `enabled` | INTEGER | Enabled flag from the legacy watch-dir projection |
| `recursive` | INTEGER | Recursive watch flag |
| `extensions` | TEXT | Optional serialized extension filter list |
| `created_at` | DATETIME | Insert timestamp |
| `profile_id` | INTEGER | Optional foreign key to `library_profiles.id` |

## `notification_targets`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `name` | TEXT | Target name |
| `target_type` | TEXT | `gotify`, `discord`, or `webhook` |
| `endpoint_url` | TEXT | Destination URL |
| `auth_token` | TEXT | Optional auth token |
| `events` | TEXT | Serialized event list |
| `enabled` | BOOLEAN | Enabled flag |
| `created_at` | DATETIME | Insert timestamp |

## `schedule_windows`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `start_time` | TEXT | Window start time |
| `end_time` | TEXT | Window end time |
| `days_of_week` | TEXT | Serialized day list |
| `enabled` | BOOLEAN | Enabled flag |

## `file_settings`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Singleton row key (`1`) |
| `delete_source` | BOOLEAN | Delete original after success |
| `output_extension` | TEXT | Output extension |
| `output_suffix` | TEXT | Filename suffix |
| `replace_strategy` | TEXT | Collision policy |
| `output_root` | TEXT | Optional mirrored output root |

## `library_profiles`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `name` | TEXT | Profile name |
| `preset` | TEXT | Preset identifier |
| `codec` | TEXT | Output codec |
| `quality_profile` | TEXT | Quality preset |
| `hdr_mode` | TEXT | HDR behavior |
| `audio_mode` | TEXT | Audio policy |
| `crf_override` | INTEGER | Optional CRF override |
| `notes` | TEXT | Optional notes |
| `created_at` | TEXT | Insert timestamp |
| `updated_at` | TEXT | Last update timestamp |

## `health_scan_runs`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `started_at` | TEXT | Scan start timestamp |
| `completed_at` | TEXT | Scan completion timestamp |
| `files_checked` | INTEGER | Files examined in the run |
| `issues_found` | INTEGER | Issues found in the run |

## `schema_info`

| Column | Type | Description |
|--------|------|-------------|
| `key` | TEXT | Primary key |
| `value` | TEXT | Version or compatibility value |

Common keys include `schema_version` and
`min_compatible_version`.

## Migration policy

Compatibility baseline: `v0.2.5`.

Migration rules:

- `CREATE TABLE IF NOT EXISTS`
- `ALTER TABLE ... ADD COLUMN` only with `NULL` allowed or a `DEFAULT`
- `CREATE INDEX IF NOT EXISTS`
- Never remove columns
- Never rename columns
- Never change column types

The policy is additive only. Existing migration files are
immutable.
