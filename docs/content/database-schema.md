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
| `input_metadata_json` | TEXT | Serialized input probe metadata captured at enqueue time so completed jobs do not require live re-probing |

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
| `reason` | TEXT | Legacy machine-readable reason string retained for compatibility |
| `reason_code` | TEXT | Stable structured explanation code |
| `reason_payload_json` | TEXT | Serialized structured explanation payload |
| `created_at` | DATETIME | Insert timestamp |

## `job_failure_explanations`

| Column | Type | Description |
|--------|------|-------------|
| `job_id` | INTEGER | Primary key and foreign key to `jobs.id` |
| `legacy_summary` | TEXT | Legacy failure summary retained for compatibility |
| `code` | TEXT | Stable structured failure code |
| `payload_json` | TEXT | Serialized structured failure explanation payload |
| `created_at` | TEXT | Insert timestamp |
| `updated_at` | TEXT | Last update timestamp |

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
| `target_type` | TEXT | Legacy target type retained for compatibility |
| `target_type_v2` | TEXT | Canonical provider type such as `discord_webhook`, `gotify`, `webhook`, `telegram`, or `email` |
| `endpoint_url` | TEXT | Legacy destination URL projection |
| `auth_token` | TEXT | Legacy auth token projection |
| `config_json` | TEXT | Provider-specific target config JSON |
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
| `custom_vfilters` | TEXT | Optional custom FFmpeg video filter chain |
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

## `conversion_jobs`

Tracks uploads from the Convert workflow and their generated outputs. Cleanup runs on every upload and is driven by `expires_at`, the linked `jobs` row state, and `downloaded_at`.

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `upload_path` | TEXT | Absolute path to the staged upload |
| `output_path` | TEXT | Absolute path to the generated output, set once the job completes |
| `mode` | TEXT | Workflow mode (e.g. `transcode`, `remux`) |
| `settings_json` | TEXT | Serialized conversion settings chosen in the UI |
| `probe_json` | TEXT | Cached FFprobe output for the upload |
| `linked_job_id` | INTEGER | Foreign key to `jobs.id` once the upload has been enqueued; nulled on job delete |
| `status` | TEXT | Current state (`uploaded`, `queued`, `running`, `completed`, `downloaded`, `failed`, `cancelled`) |
| `expires_at` | TEXT | Absolute timestamp after which the cleanup sweep may remove artifacts |
| `downloaded_at` | TEXT | Download timestamp; cleanup extends `expires_at` by `conversion_download_retention_hours` once this is set |
| `created_at` | TEXT | Insert timestamp |
| `updated_at` | TEXT | Last update timestamp |

## `media_probe_cache`

Caches FFprobe analysis for unchanged files. The analyzer
uses the cache when the input path, mtime, size, and probe
version all match.

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `input_path` | TEXT | Absolute path that was probed |
| `mtime_ns` | INTEGER | File modification time fingerprint |
| `size_bytes` | INTEGER | File size fingerprint |
| `probe_version` | TEXT | FFprobe version marker |
| `analysis_json` | TEXT | Serialized analyzer result |
| `created_at` | DATETIME | Insert timestamp |
| `updated_at` | DATETIME | Last cache write timestamp |
| `last_accessed_at` | DATETIME | Last successful cache read timestamp |

`(input_path, mtime_ns, size_bytes, probe_version)` is unique.

## `hardware_detection_cache`

Stores the last successful hardware detection result so boot
can reuse it when the machine/runtime fingerprint is still
valid.

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Singleton row key (`1`) |
| `cache_key` | TEXT | Hash of the hardware detection fingerprint |
| `fingerprint_json` | TEXT | OS, architecture, FFmpeg/FFprobe versions, hardware settings, and cache schema version |
| `hardware_info_json` | TEXT | Serialized selected hardware backend and supported codecs |
| `probe_log_json` | TEXT | Serialized probe log shown in Settings -> Hardware |
| `detected_at` | DATETIME | Time this detection result was produced |
| `updated_at` | DATETIME | Last cache write timestamp |

## `api_tokens`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `name` | TEXT | Human-readable token label |
| `token_hash` | TEXT | Hashed token value; the plaintext is shown once at issue |
| `access_level` | TEXT | Stored access class (`read_only` or `full_access`) |
| `access_scope` | TEXT | Optional narrowed scope (currently `arr_webhook` for ARR-only webhook tokens) |
| `created_at` | DATETIME | Insert timestamp |
| `last_used_at` | DATETIME | Updated on each successful authenticated request |
| `revoked_at` | DATETIME | Non-null once the token is revoked |

## `encode_attempts`

Per-attempt encode history. A job may have multiple rows if it was retried.

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `job_id` | INTEGER | Foreign key to `jobs.id`, cascades on delete |
| `attempt_number` | INTEGER | 1-based attempt index |
| `started_at` | TEXT | Attempt start timestamp |
| `finished_at` | TEXT | Attempt finish timestamp |
| `outcome` | TEXT | `completed`, `failed`, or `cancelled` |
| `failure_code` | TEXT | Stable structured failure code for failed attempts |
| `failure_summary` | TEXT | Legacy failure summary string |
| `input_size_bytes` | INTEGER | Input size at attempt time |
| `output_size_bytes` | INTEGER | Output size at attempt time |
| `encode_time_seconds` | REAL | Wall-clock encode duration |
| `created_at` | TEXT | Insert timestamp |

## `job_resume_sessions`

Tracks resumable segmented encodes so long-running jobs can pick up after restarts.

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `job_id` | INTEGER | Unique foreign key to `jobs.id`, cascades on delete |
| `strategy` | TEXT | Resume strategy identifier |
| `plan_hash` | TEXT | Hash of the encode plan; invalidates the session if plan changes |
| `mtime_hash` | TEXT | Input mtime fingerprint; invalidates on source change |
| `temp_dir` | TEXT | Per-job temporary directory for segment outputs |
| `concat_manifest_path` | TEXT | FFmpeg concat manifest path |
| `segment_length_secs` | INTEGER | Segment duration in seconds |
| `status` | TEXT | `active`, `completed`, or `abandoned` |
| `created_at` | DATETIME | Insert timestamp |
| `updated_at` | DATETIME | Last update timestamp |

## `job_resume_segments`

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `job_id` | INTEGER | Foreign key to `jobs.id`, cascades on delete |
| `segment_index` | INTEGER | Segment order within the job |
| `start_secs` | REAL | Segment start offset in seconds |
| `duration_secs` | REAL | Segment duration in seconds |
| `temp_path` | TEXT | Path to the encoded segment on disk |
| `status` | TEXT | Segment state (`pending`, `in_progress`, `completed`, `failed`) |
| `attempt_count` | INTEGER | Retry count for this segment |
| `created_at` | DATETIME | Insert timestamp |
| `updated_at` | DATETIME | Last update timestamp |

`(job_id, segment_index)` is unique.

## `schema_info`

| Column | Type | Description |
|--------|------|-------------|
| `key` | TEXT | Primary key |
| `value` | TEXT | Version or compatibility value |

Common keys include `schema_version` (`13` in
`0.3.2-rc.2`) and `min_compatible_version`.

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
