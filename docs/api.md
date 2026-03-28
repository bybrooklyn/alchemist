---
title: API
description: REST and SSE API reference for Alchemist.
---

All API routes require the `alchemist_session` auth cookie
except:

- `/api/auth/*`
- `/api/health`
- `/api/ready`
- setup-mode exceptions: `/api/setup/*`, `/api/fs/*`,
  `/api/settings/bundle`, `/api/system/hardware`

Authentication is established by `POST /api/auth/login`.
The backend also accepts `Authorization: Bearer <token>`,
but the web UI uses the session cookie.

## Authentication

### `POST /api/auth/login`

Request:

```json
{
  "username": "admin",
  "password": "secret"
}
```

Response:

```http
HTTP/1.1 200 OK
Set-Cookie: alchemist_session=...; HttpOnly; SameSite=Lax; Path=/; Max-Age=2592000
```

```json
{
  "status": "ok"
}
```

### `POST /api/auth/logout`

Clears the session cookie and deletes the server-side
session if one exists.

```json
{
  "status": "ok"
}
```

## Jobs

### `GET /api/jobs`

Canonical job listing endpoint. Supports query params such
as `limit`, `page`, `status`, `search`, `sort_by`,
`sort_desc`, and `archived`.

Example:

```bash
curl -b cookie.txt \
  'http://localhost:3000/api/jobs?status=queued,failed&limit=50&page=1'
```

### `GET /api/jobs/:id/details`

Returns the job row, any available analyzed metadata,
encode stats for completed jobs, recent job logs, and a
failure summary for failed jobs.

Example response shape:

```json
{
  "job": {
    "id": 42,
    "input_path": "/media/movies/example.mkv",
    "status": "completed"
  },
  "metadata": {
    "codec_name": "h264",
    "width": 1920,
    "height": 1080
  },
  "encode_stats": {
    "input_size_bytes": 8011223344,
    "output_size_bytes": 4112233445,
    "compression_ratio": 1.95,
    "encode_speed": 2.4,
    "vmaf_score": 93.1
  },
  "job_logs": [],
  "job_failure_summary": null
}
```

### `POST /api/jobs/:id/cancel`

Cancels a queued or active job if the current state allows
it.

### `POST /api/jobs/:id/restart`

Restarts a non-active job by sending it back to `queued`.

### `POST /api/jobs/:id/priority`

Request:

```json
{
  "priority": 100
}
```

Response:

```json
{
  "id": 42,
  "priority": 100
}
```

### `POST /api/jobs/batch`

Supported `action` values: `cancel`, `restart`, `delete`.

```json
{
  "action": "restart",
  "ids": [41, 42, 43]
}
```

Response:

```json
{
  "count": 3
}
```

### `POST /api/jobs/restart-failed`

Response:

```json
{
  "count": 2,
  "message": "Queued 2 failed or cancelled jobs for retry."
}
```

### `POST /api/jobs/clear-completed`

Archives completed jobs from the visible queue while
preserving historical encode stats.

```json
{
  "count": 12,
  "message": "Cleared 12 completed jobs from the queue. Historical stats were preserved."
}
```

## Engine

### `POST /api/engine/pause`

```json
{
  "status": "paused"
}
```

### `POST /api/engine/resume`

```json
{
  "status": "running"
}
```

### `POST /api/engine/drain`

```json
{
  "status": "draining"
}
```

### `POST /api/engine/stop-drain`

```json
{
  "status": "running"
}
```

### `GET /api/engine/status`

Response fields:

- `status`
- `mode`
- `concurrent_limit`
- `manual_paused`
- `scheduler_paused`
- `draining`
- `is_manual_override`

Example:

```json
{
  "status": "paused",
  "manual_paused": true,
  "scheduler_paused": false,
  "draining": false,
  "mode": "balanced",
  "concurrent_limit": 2,
  "is_manual_override": false
}
```

### `GET /api/engine/mode`

Returns current mode, whether a manual override is active,
the current concurrent limit, CPU count, and computed mode
limits.

### `POST /api/engine/mode`

Request:

```json
{
  "mode": "balanced",
  "concurrent_jobs_override": 2,
  "threads_override": 0
}
```

Response:

```json
{
  "status": "ok",
  "mode": "balanced",
  "concurrent_limit": 2,
  "is_manual_override": true
}
```

## Stats

### `GET /api/stats/aggregated`

```json
{
  "total_input_bytes": 1234567890,
  "total_output_bytes": 678901234,
  "total_savings_bytes": 555666656,
  "total_time_seconds": 81234.5,
  "total_jobs": 87,
  "avg_vmaf": 92.4
}
```

### `GET /api/stats/daily`

Returns the last 30 days of encode activity.

### `GET /api/stats/detailed`

Returns the most recent detailed encode stats rows.

### `GET /api/stats/savings`

Returns the storage-savings summary used by the statistics
dashboard.

## Settings

### `GET /api/settings/transcode`

Returns the transcode settings payload currently loaded by
the backend.

### `POST /api/settings/transcode`

Request:

```json
{
  "concurrent_jobs": 2,
  "size_reduction_threshold": 0.3,
  "min_bpp_threshold": 0.1,
  "min_file_size_mb": 50,
  "output_codec": "av1",
  "quality_profile": "balanced",
  "threads": 0,
  "allow_fallback": true,
  "hdr_mode": "preserve",
  "tonemap_algorithm": "hable",
  "tonemap_peak": 100.0,
  "tonemap_desat": 0.2,
  "subtitle_mode": "copy",
  "stream_rules": {
    "strip_audio_by_title": ["commentary"],
    "keep_audio_languages": ["eng"],
    "keep_only_default_audio": false
  }
}
```

## System

### `GET /api/system/hardware`

Returns the current detected hardware backend, supported
codecs, backends, and any detection notes.

### `GET /api/system/hardware/probe-log`

Returns the per-encoder probe log with success/failure
status and stderr excerpts.

### `GET /api/system/resources`

Returns live resource data:

- `cpu_percent`
- `memory_used_mb`
- `memory_total_mb`
- `memory_percent`
- `uptime_seconds`
- `active_jobs`
- `concurrent_limit`
- `cpu_count`
- `gpu_utilization`
- `gpu_memory_percent`

## Server-Sent Events

### `GET /api/events`

Internal event types are `JobStateChanged`, `Progress`,
`Decision`, and `Log`. The SSE stream exposed to clients
emits lower-case event names:

- `status`
- `progress`
- `decision`
- `log`

Additional config/system events may also appear, including
`config_updated`, `scan_started`, `scan_completed`,
`engine_status_changed`, and `hardware_state_changed`.

Example:

```text
event: progress
data: {"job_id":42,"percentage":61.4,"time":"00:11:32"}
```
