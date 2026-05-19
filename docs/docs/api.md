---
title: API Reference
description: REST and SSE API reference for Alchemist.
---

## Authentication

Canonical client routes live under `/api/v1`. The older `/api` routes remain
available as compatibility aliases for the web UI and existing scripts, but new
external clients should use `/api/v1`.

All protected API routes require the `alchemist_session` auth cookie established
via `POST /api/v1/auth/login`, or an `Authorization: Bearer <token>` header.

Machine-readable contract: [OpenAPI spec](/openapi.yaml)

Maintainers should run `just api-contract` after changing API routes. The
release checklist runs the same contract check.

The stdio Model Context Protocol server is documented separately in
[MCP Server](/mcp). It is read-only and does not expose HTTP mutation routes.

API errors use `application/problem+json`, include an `X-Request-Id` response
header, and retain the legacy `error.code` / `error.message` object for older
client parsers:

```json
{
  "type": "urn:alchemist:problem:auth-required",
  "title": "Unauthorized",
  "status": 401,
  "detail": "Unauthorized",
  "instance": "/api/v1/engine/status",
  "code": "AUTH_REQUIRED",
  "request_id": "2c168dbf-6f29-41eb-a8e7-0fd563f76fd4",
  "error": {
    "code": "AUTH_REQUIRED",
    "message": "Unauthorized"
  }
}
```

Mutation routes that do not return a resource return a typed JSON body:

```json
{ "ok": true }
```

### `POST /api/v1/auth/login`
Establish a session. Returns a `Set-Cookie` header.

**Request:**
```json
{
  "username": "admin",
  "password": "..."
}
```

### `POST /api/v1/auth/logout`
Invalidate current session and clear cookie.

### `GET /api/v1/settings/api-tokens`
List metadata for configured API tokens.

### `POST /api/v1/settings/api-tokens`
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
- `arr_webhook` — only `POST /api/v1/webhooks/arr`
- `jellyfin` — Jellyfin plugin endpoints: system info, readiness, SSE events,
  job details, and `POST /api/v1/jobs/enqueue`
- `full_access` — all authenticated API routes

### `DELETE /api/v1/settings/api-tokens/:id`
Revoke a token.

---

## Settings

### `GET /api/v1/settings/bundle`
Fetch the full settings projection used by setup and the
settings UI.

### `PUT /api/v1/settings/bundle`
Persist the full settings bundle. Fails with `409` when
`ALCHEMIST_CONFIG_MUTABLE=false`.

### `POST /api/v1/settings/config/validate`
Parse and validate candidate TOML without saving it. Returns a
redacted summary and conservative warnings for high-risk
settings.

### `GET|POST /api/v1/settings/system`
Read or update runtime-facing system settings:
conversion upload limit, converted-download retention,
telemetry switch, engine mode, metrics switch, update
channel/check settings, and ARR path translations.

### `GET|POST /api/v1/settings/hardware`
Read or update hardware preference, device path, CPU
fallback, CPU encoding, and CPU preset. Updating hardware
settings refreshes runtime hardware state and cache.

### `GET|PUT|POST /api/v1/settings/notifications`
Read notification schedule/quiet-hours state, update global
notification timing, or add a target.

Global fields:

- `daily_summary_time_local`
- `quiet_hours_enabled`
- `quiet_hours_start_local`
- `quiet_hours_end_local`

Targets use `target_type`, provider-specific `config_json`,
`events`, and `enabled`.

### `POST /api/v1/settings/notifications/test`
Send a test notification using a target payload without
saving it.

---

## ARR webhook ingress

### `POST /api/v1/webhooks/arr`
Accepts Sonarr/Radarr webhook payloads with `eventType=Download`, resolves a
media path, applies optional `system.arr_path_translations`, then reuses the
standard enqueue-by-path pipeline (same dedupe/output guards as manual enqueue).

Webhook setup:

1. Create an API token with `access_level: "arr_webhook"`.
2. In Sonarr/Radarr add a **Webhook** notification pointing to:
   `http://<alchemist-host>:3000/api/v1/webhooks/arr`
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

## Jellyfin plugin integration

The Jellyfin plugin uses a dedicated `jellyfin` API token. That token can enqueue
media, keep an SSE connection open, and fetch completed job details without
granting full settings or engine-control access.

Allowed routes:

- `GET /api/v1/system/info`
- `GET /api/v1/ready`
- `GET /api/v1/events`
- `GET /api/v1/jobs/:id/details`
- `POST /api/v1/jobs/enqueue`

Plugin setup:

1. Create an API token with `access_level: "jellyfin"`.
2. Configure the plugin with the Alchemist URL and token.
3. Add forward path translations for Jellyfin-to-Alchemist enqueue paths.
4. Add reverse path translations for Alchemist-to-Jellyfin refresh paths, or
   leave them empty to invert the forward translations.
5. Enable the event listener and refresh-on-completion options when ready.

---

## Conversion

The Convert workflow is an experimental single-file utility.
It reuses the normal analyzer/planner/executor path, but
tracks staged uploads in `conversion_jobs`.

### `POST /api/v1/conversion/uploads`
Upload one source file. The maximum size is
`system.conversion_upload_limit_gb` GiB.

### `POST /api/v1/conversion/preview`
Return normalized settings, generated FFmpeg command text,
and a structured source/output/estimate summary.

### `POST /api/v1/conversion/jobs/:id/start`
Queue the uploaded conversion job.

### `GET /api/v1/conversion/jobs/:id`
Fetch current conversion job state.

### `GET /api/v1/conversion/jobs/:id/download`
Download completed output. After download, cleanup retention
is governed by `system.conversion_download_retention_hours`.

### `DELETE /api/v1/conversion/jobs/:id`
Delete the conversion record and any staged artifacts that
are safe to remove.

---

## Jobs

### `GET /api/v1/jobs`
List jobs with filtering and pagination.

**Params:** `limit`, `page`, `status`, `search`, `sort_by`, `sort_desc`, `archived`.
The `search` value matches job paths plus stored decision and failure explanation text.

### `GET /api/v1/jobs/:id/details`
Fetch full job state, metadata, logs, and stats.

### `DELETE /api/v1/jobs/:id`
Delete a terminal job. The legacy alias remains
`POST /api/jobs/:id/delete`.

### `POST /api/v1/jobs/:id/cancel`
Cancel a queued or active job.

### `POST /api/v1/jobs/:id/restart`
Restart a terminal job (failed/cancelled/completed).

### `POST /api/v1/jobs/:id/priority`
Update job priority.

**Request:** `{"priority": 100}`

### `POST /api/v1/jobs/batch`
Bulk action on multiple jobs.

**Request:**
```json
{
  "action": "restart|cancel|delete",
  "ids": [1, 2, 3]
}
```

### `POST /api/v1/jobs/restart-failed`
Restart all failed or cancelled jobs.

### `POST /api/v1/jobs/clear-completed`
Archive all completed jobs from the active queue.

### `POST /api/v1/jobs/clear-history`
Archive terminal completed, failed, cancelled, and skipped
jobs from the active table.

---

## Engine

### `GET /api/v1/engine/status`
Get current operational status and limits.

Response fields include:

- `status`: `running`, `paused`, or `draining`
- `manual_paused`
- `scheduler_paused`
- `draining`
- `blocked_reason`: `manual_paused`, `scheduled_pause`,
  `draining`, `workers_busy`, or `null`

### `POST /api/v1/engine/pause`
Pause the engine. Active jobs continue; no new jobs are
claimed.

### `POST /api/v1/engine/resume`
Resume the engine.

### `POST /api/v1/engine/drain`
Enter drain mode (finish active jobs, don't start new ones).

### `POST /api/v1/engine/stop-drain`
Cancel drain mode without changing scheduler pause state.

### `POST /api/v1/engine/restart`
Pause briefly, cancel in-flight work, clear drain state, then
resume claiming jobs.

### `POST /api/v1/engine/mode`
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

### `GET /api/v1/stats/aggregated`
Total savings, job counts, and global efficiency.

### `GET /api/v1/stats/queue-eta`
Queue-wide ETA estimate from recent completed encode samples.

Returns:
```json
{
  "remaining_jobs": 3,
  "est_seconds_remaining": 3600,
  "sample_size": 4
}
```

### `GET /api/v1/stats/daily`
Encode activity history for the last 30 days.

### `GET /api/v1/stats/savings`
Detailed breakdown of storage savings.

---

## System

### `GET /api/v1/system/hardware`
Detected hardware backend and codec support matrix. During
startup this may be served from a valid hardware detection
cache before a background refresh updates runtime state.

### `GET /api/v1/system/hardware/probe-log`
Full logs from the startup hardware probe.

### `GET /api/v1/system/resources`
Live telemetry: CPU, Memory, GPU utilization, and uptime.

### `GET /api/v1/system/update`
Return update metadata for the configured channel (`stable`,
`rc`, or `nightly`), including install type, verification
status, self-update eligibility, and package-manager guidance.

### `POST /api/v1/system/update/check`
Force-refresh update metadata from GitHub Releases.

### `POST /api/v1/system/update/install`
Start a verified update for eligible direct Linux/macOS binary
installs. Active jobs trigger drain mode first. Docker,
Homebrew, AUR, Windows, source, and unknown installs return
guided update instructions instead of replacing files.

### `POST /api/v1/system/backup`
Download a gzip-compressed SQLite backup produced through
SQLite's online backup path. Requires full access.

### `POST /api/v1/system/backup/validate-restore`
Validate an uploaded `.db.gz` snapshot before restore. The
endpoint decompresses the backup to a temporary file, verifies
that it is an Alchemist SQLite database, and returns schema
metadata without mutating the live database. Requires full
access.

### `GET /api/v1/ready`
Readiness check. Returns `503` with structured error code
`DATABASE_UNAVAILABLE` if SQLite is not usable.

### `GET /metrics`
Prometheus metrics endpoint. Disabled unless
`system.metrics_enabled=true`.

---

## Library

### `GET /api/v1/library/intelligence`
Storage-focused recommendations: duplicates, remux
opportunities, wasteful audio layouts, and
commentary/descriptive-track cleanup candidates.

### `GET /api/v1/library/health`
Current Library Doctor summary.

### `POST /api/v1/library/health/scan`
Start a full health scan. Overlapping full-library scans are
rejected.

### `GET /api/v1/library/health/issues`
List current health issues.

---

## Events (SSE)

### `GET /api/v1/events`
Real-time event stream. 

**Emitted Events:**
- `status`: Job state changes.
- `progress`: Real-time encode statistics.
- `decision`: Skip/Transcode logic results.
- `log`: Engine and job logs.
- `config_updated`: Configuration hot-reload notification.
- `scan_started` / `scan_completed`: Library scan status.
