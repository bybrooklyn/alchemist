---
title: API Reference
description: REST and SSE API reference for Alchemist.
---

## Authentication

All API routes require the `alchemist_session` auth cookie established via `/api/auth/login`, or an `Authorization: Bearer <token>` header. 

Machine-readable contract: [OpenAPI spec](/openapi.yaml)

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

---

## Engine

### `GET /api/engine/status`
Get current operational status and limits.

### `POST /api/engine/pause`
Pause the engine (suspend active jobs).

### `POST /api/engine/resume`
Resume the engine.

### `POST /api/engine/drain`
Enter drain mode (finish active jobs, don't start new ones).

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
Detected hardware backend and codec support matrix.

### `GET /api/system/hardware/probe-log`
Full logs from the startup hardware probe.

### `GET /api/system/resources`
Live telemetry: CPU, Memory, GPU utilization, and uptime.

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
