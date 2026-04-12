---
title: Engine Lifecycle
description: Engine states, transitions, and job cancellation semantics.
---

The Alchemist engine is a background loop that claims queued jobs, processes them, and manages concurrent execution. This page documents all states, what triggers each transition, and the exact behavior during cancel, pause, drain, and restart.

---

## Engine states

| State | Jobs start? | Active jobs affected? | How to enter |
|-------|------------|----------------------|-------------|
| **Running** | Yes | Not affected | Resume, restart |
| **Paused** (manual) | No | Not cancelled | Header → Stop, `POST /api/engine/pause` |
| **Paused** (scheduler) | No | Not cancelled | Schedule window activates |
| **Draining** | No | Run to completion | Header → Stop (while running), `POST /api/engine/drain` |
| **Restarting** | No (briefly) | Cancelled | `POST /api/engine/restart` |
| **Shutdown** | No | Force-cancelled | Process exit / SIGTERM |

Paused-manual and paused-scheduler are independent. Both must be cleared for jobs to start again.

---

## State transitions

```
             Resume
  ┌──────────────────────────────┐
  │                              ▼
Paused ◄─── Pause ─────── Running ──── Drain ───► Draining
  │                         ▲  │                     │
  │         Restart          │  └─── Shutdown ──►  Shutdown
  │      ┌──────────┐        │
  └─────►│ Restart  │────────┘
         └──────────┘
         (brief pause,
         cancel in-flight,
         then resume)
```

### Pause

- Sets `manual_paused = true`.
- The claim loop polls every 2 seconds and blocks while paused.
- Active jobs continue until they finish naturally.
- Does **not** affect draining state.

### Resume

- Clears `manual_paused`.
- Does **not** clear `scheduler_paused` (scheduler manages its own flag).
- The claim loop immediately resumes on the next iteration.
- Does **not** cancel the drain if draining.

### Drain

- Sets `draining = true` without setting `paused`.
- No new jobs are claimed.
- Active jobs run to completion.
- When `in_flight_jobs` reaches zero: drain completes, `draining` is cleared, engine transitions to **Paused** (manual).

### Restart

1. Pause (set `manual_paused = true`).
2. Cancel all in-flight jobs (Encoding, Remuxing, Analyzing, Resuming) via FFmpeg kill signal.
3. Clear `draining` flag.
4. Clear `idle_notified` flag.
5. Resume (clear `manual_paused`).

Cancelled in-flight jobs are marked `failed` with `failure_summary = "cancelled"`. They are eligible for automatic retry per the retry backoff schedule.

### Shutdown

Called when the process exits (SIGTERM / graceful shutdown):

1. Cancel all active jobs via FFmpeg kill.
2. Wait up to a short timeout for kills to complete.
3. No retry is scheduled — the jobs return to `queued` on next startup.

---

## Job states

| Job state | Meaning | Terminal? |
|-----------|---------|-----------|
| `queued` | Waiting to be claimed | No |
| `analyzing` | FFprobe running on the file | No |
| `encoding` | FFmpeg encoding in progress | No |
| `remuxing` | FFmpeg stream-copy in progress | No |
| `resuming` | Job being re-queued after retry | No |
| `completed` | Encode finished successfully | Yes |
| `skipped` | Planner decided not to transcode | Yes |
| `failed` | Encode or analysis failed | Yes (with retry) |
| `cancelled` | Cancelled by operator | Yes (with retry) |

---

## Retry backoff

Failed and cancelled jobs are automatically retried. The engine checks elapsed time before claiming.

| Attempt # | Backoff before retry |
|-----------|---------------------|
| 1 | 5 minutes |
| 2 | 15 minutes |
| 3 | 60 minutes |
| 4+ | 6 hours |

After 3 consecutive failures with no success, the job still retries on the 6-hour schedule. There is no permanent failure state from retries alone — operator must manually delete or cancel the job to stop retries.

---

## Cancel semantics

### Cancel mid-analysis

FFprobe process is not currently cancellable via signal. The cancel flag is checked before FFprobe starts. If analysis is in progress when cancel arrives, the job will be cancelled after analysis completes (before encoding starts).

### Cancel mid-encode

The FFmpeg process receives a kill signal immediately. The partial output file is cleaned up. The job is marked `failed` with `failure_summary = "cancelled"`.

### Cancel while queued

The job status is set to `cancelled` directly without any process kill.

---

## Pause vs. drain vs. restart

| Operation | In-flight jobs | Partial output | New jobs |
|-----------|---------------|---------------|----------|
| Pause | Finish normally | Not affected | Blocked |
| Drain | Finish normally | Not affected | Blocked until drain completes |
| Restart | Killed | Cleaned up | Blocked briefly, then resume |
| Shutdown | Killed | Cleaned up | N/A |

Use **Pause** when you need to inspect the queue or change settings without losing progress.

Use **Drain** when you want to stop gracefully after the current batch finishes (e.g. before maintenance).

Use **Restart** to force a clean slate — e.g. after changing hardware settings that affect in-flight jobs.

---

## Boot sequence

1. Migrations run.
2. Any jobs left in `encoding`, `remuxing`, `analyzing`, or `resuming` are reset to `queued` (crash recovery).
3. Boot analysis runs — all `queued` jobs that have no metadata have FFprobe run on them. This uses a single-slot semaphore and blocks the claim loop.
4. Engine claim loop starts — jobs are claimed and processed up to the concurrent limit.
