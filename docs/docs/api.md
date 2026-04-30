---
title: API Reference
description: REST and SSE API reference for Alchemist.
---

## Authentication

All API routes require the `alchemist_session` auth cookie established via `/api/auth/login`, or an `Authorization: Bearer <token>` header. 

Machine-readable contract: [OpenAPI spec](/openapi.yaml)

Most API errors use this envelope:

```json
{
  "error": {
    "code": "CONFIG_SAVE_FAILED",
    "message": "Failed to save configuration"
  }
}
```

Older routes may still have legacy shapes, but high-traffic
auth, settings, jobs, system, ARR webhook, readiness, SSE,
and metrics failure paths now use stable machine-readable
codes.

### `POST /api/auth/login`
Establish a session. Returns a `Set-Cookie` header.

**Request:**
```json
{
  "username": "admin",
  "password": "..."
}
```

### `POST /api/auth/logout`
Invalidate current session and clear cookie.

### `GET /api/settings/api-tokens`
List metadata for configured API tokens.

### `POST /api/settings/api-tokens`
Create a new API token. The plaintext value is only returned once.

**Request:**
```json
{
  "name": "Prometheus",
  "access_level": "read_only"
}
```

`access_level` supports:

- `read_only` — observability GET/HEAD endpoints only
- `arr_webhook` — only `POST /api/webhooks/arr`
- `full_access` — all authenticated API routes

### `DELETE /api/settings/api-tokens/:id`
Revoke a token.

---

## Settings

### `GET /api/settings/bundle`
Fetch the full settings projection used by setup and the
settings UI.

### `PUT /api/settings/bundle`
Persist the full settings bundle. Fails with `409` when
`ALCHEMIST_CONFIG_MUTABLE=false`.

### `GET|POST /api/settings/system`
Read or update runtime-facing system settings:
conversion upload limit, converted-download retention,
telemetry switch, engine mode, metrics switch, and ARR path
translations.

### `GET|POST /api/settings/hardware`
Read or update hardware preference, device path, CPU
fallback, CPU encoding, and CPU preset. Updating hardware
settings refreshes runtime hardware state and cache.

### `GET|PUT|POST /api/settings/notifications`
Read notification schedule/quiet-hours state, update global
notification timing, or add a target.

Global fields:

- `daily_summary_time_local`
- `quiet_hours_enabled`
- `quiet_hours_start_local`
- `quiet_hours_end_local`

Targets use `target_type`, provider-specific `config_json`,
`events`, and `enabled`.

### `POST /api/settings/notifications/test`
Send a test notification using a target payload without
saving it.

---

## ARR webhook ingress

### `POST /api/webhooks/arr`
Accepts Sonarr/Radarr webhook payloads with `eventType=Download`, resolves a
media path, applies optional `system.arr_path_translations`, then reuses the
standard enqueue-by-path pipeline (same dedupe/output guards as manual enqueue).

Webhook setup:

1. Create an API token with `access_level: "arr_webhook"`.
2. In Sonarr/Radarr add a **Webhook** notification pointing to:
   `http://<alchemist-host>:3000/api/webhooks/arr`
3. Add header: `Authorization: Bearer <token>`.
4. Ensure the payload includes import path fields (`episodeFile`, `movieFile`,
   `importedEpisodeFiles`, or `importedMovieFiles`).

Container path mapping (if Arr sees different mount paths than Alchemist):

```toml
[system]
arr_path_translations = [
  { from = "/container/media", to = "/mnt/media" }
]
```

---

## Conversion

The Convert workflow is an experimental single-file utility.
It reuses the normal analyzer/planner/executor path, but
tracks staged uploads in `conversion_jobs`.

### `POST /api/conversion/uploads`
Upload one source file. The maximum size is
`system.conversion_upload_limit_gb` GiB.

### `POST /api/conversion/preview`
Return normalized settings, generated FFmpeg command text,
and a structured source/output/estimate summary.

### `POST /api/conversion/jobs/:id/start`
Queue the uploaded conversion job.

### `GET /api/conversion/jobs/:id`
Fetch current conversion job state.

### `GET /api/conversion/jobs/:id/download`
Download completed output. After download, cleanup retention
is governed by `system.conversion_download_retention_hours`.

### `DELETE /api/conversion/jobs/:id`
Delete the conversion record and any staged artifacts that
are safe to remove.

---

## Jobs

### `GET /api/jobs`
List jobs with filtering and pagination.

**Params:** `limit`, `page`, `status`, `search`, `sort_by`, `sort_desc`, `archived`.

### `GET /api/jobs/:id/details`
Fetch full job state, metadata, logs, and stats.

### `POST /api/jobs/:id/cancel`
Cancel a queued or active job.

### `POST /api/jobs/:id/restart`
Restart a terminal job (failed/cancelled/completed).

### `POST /api/jobs/:id/priority`
Update job priority.

**Request:** `{"priority": 100}`

### `POST /api/jobs/batch`
Bulk action on multiple jobs.

**Request:**
```json
{
  "action": "restart|cancel|delete",
  "ids": [1, 2, 3]
}
```

### `POST /api/jobs/restart-failed`
Restart all failed or cancelled jobs.

### `POST /api/jobs/clear-completed`
Archive all completed jobs from the active queue.

### `POST /api/jobs/clear-history`
Archive terminal completed, failed, cancelled, and skipped
jobs from the active table.

---

## Engine

### `GET /api/engine/status`
Get current operational status and limits.

Response fields include:

- `status`: `running`, `paused`, or `draining`
- `manual_paused`
- `scheduler_paused`
- `draining`
- `blocked_reason`: `manual_paused`, `scheduled_pause`,
  `draining`, `workers_busy`, or `null`

### `POST /api/engine/pause`
Pause the engine. Active jobs continue; no new jobs are
claimed.

### `POST /api/engine/resume`
Resume the engine.

### `POST /api/engine/drain`
Enter drain mode (finish active jobs, don't start new ones).

### `POST /api/engine/stop-drain`
Cancel drain mode without changing scheduler pause state.

### `POST /api/engine/restart`
Pause briefly, cancel in-flight work, clear drain state, then
resume claiming jobs.

### `POST /api/engine/mode`
Switch engine mode or apply manual overrides.

**Request:**
```json
{
  "mode": "background|balanced|throughput",
  "concurrent_jobs_override": 2,
  "threads_override": 0
}
```

---

## Statistics

### `GET /api/stats/aggregated`
Total savings, job counts, and global efficiency.

### `GET /api/stats/daily`
Encode activity history for the last 30 days.

### `GET /api/stats/savings`
Detailed breakdown of storage savings.

---

## System

### `GET /api/system/hardware`
Detected hardware backend and codec support matrix. During
startup this may be served from a valid hardware detection
cache before a background refresh updates runtime state.

### `GET /api/system/hardware/probe-log`
Full logs from the startup hardware probe.

### `GET /api/system/resources`
Live telemetry: CPU, Memory, GPU utilization, and uptime.

### `POST /api/system/backup`
Download a gzip-compressed SQLite backup produced through
SQLite's online backup path. Requires full access.

### `GET /api/ready`
Readiness check. Returns `503` with structured error code
`DATABASE_UNAVAILABLE` if SQLite is not usable.

### `GET /metrics`
Prometheus metrics endpoint. Disabled unless
`system.metrics_enabled=true`.

---

## Library

### `GET /api/library/intelligence`
Storage-focused recommendations: duplicates, remux
opportunities, wasteful audio layouts, and
commentary/descriptive-track cleanup candidates.

### `GET /api/library/health`
Current Library Doctor summary.

### `POST /api/library/health/scan`
Start a full health scan. Overlapping full-library scans are
rejected.

### `GET /api/library/health/issues`
List current health issues.

---

## Events (SSE)

### `GET /api/events`
Real-time event stream. 

**Emitted Events:**
- `status`: Job state changes.
- `progress`: Real-time encode statistics.
- `decision`: Skip/Transcode logic results.
- `log`: Engine and job logs.
- `config_updated`: Configuration hot-reload notification.
- `scan_started` / `scan_completed`: Library scan status.
