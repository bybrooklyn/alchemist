---
title: Architecture
description: Internal pipeline, state machine, and source layout.
---

Alchemist is a single Rust application that serves the API,
embeds the frontend, runs the scan/plan/encode pipeline, and
persists state in SQLite.

## Pipeline

```text
Scanner
  -> Agent
  -> FfmpegAnalyzer
  -> BasicPlanner
  -> FfmpegExecutor
  -> post-encode checks and promotion
  -> database update
```

Practical flow:

1. `Scanner` finds files and enqueues jobs.
2. `Agent` in `src/media/processor.rs` claims queued jobs
   and applies engine-state and concurrency rules.
3. `FfmpegAnalyzer` runs `ffprobe` with a 120-second timeout
   and builds normalized media metadata.
4. `BasicPlanner` decides skip, remux, or transcode and
   selects the best available encoder.
5. `FfmpegExecutor` runs FFmpeg.
6. Post-encode logic optionally runs VMAF, promotes the temp
   output, records decisions and stats, and updates job state.

## Engine state machine

States:

- `Running`
- `Paused`
- `Draining`
- `SchedulerPaused`

Behavior:

- `Running`: jobs start up to the active concurrency limit
- `Paused`: no new jobs start; active jobs continue
- `Draining`: active jobs finish; no new jobs start
- `SchedulerPaused`: pause state enforced by schedule windows

## Engine modes

| Mode | Formula |
|------|---------|
| Background | `1` |
| Balanced | `floor(cpu_count / 2)`, minimum `1`, maximum `4` |
| Throughput | `floor(cpu_count / 2)`, minimum `1`, uncapped |

Manual concurrency overrides can replace the computed limit
without changing the mode.

## Source layout

### `src/server/`

- `mod.rs`: `AppState`, router assembly, static asset serving
- `auth.rs`: login, logout, session cookies
- `jobs.rs`: queue endpoints, engine control, job details
- `scan.rs`: manual scan endpoints
- `settings.rs`: config and projection handlers
- `stats.rs`: stats and savings endpoints
- `system.rs`: health, readiness, resources, hardware, setup FS helpers
- `sse.rs`: SSE multiplexing
- `middleware.rs`: auth, security headers, rate limiting
- `wizard.rs`: first-run setup flow

### `src/media/`

- `pipeline.rs`: pipeline interfaces and plan types
- `planner.rs`: `BasicPlanner`, skip/remux/transcode decisions
- `analyzer.rs`: FFprobe wrapper with 120-second timeout
- `executor.rs`: FFmpeg execution path
- `processor.rs`: `Agent` loop and engine-state handling
- `scanner.rs`: filesystem scanning
- `health.rs`: Library Doctor checks
- `ffmpeg/`: encoder-specific FFmpeg builders

### `src/db/`

SQLite access layer, migration runner, and typed projections.
Split into focused submodules — no ORM, direct `sqlx` usage:

- `mod.rs`: connection pool, migrations, shared setup
- `types.rs`: row structs shared across modules
- `jobs.rs`: queue mutations, lifecycle, archival, health-check sweeps
- `conversion.rs`: Convert-workflow upload/output tracking and cleanup queries
- `stats.rs`: aggregates, savings history, daily rollups
- `config.rs`: persisted settings projections
- `system.rs`: watch dirs, library profiles, schema/version info, API tokens
- `probe_cache.rs`: FFprobe result cache keyed by path, mtime, size, and probe version
- `hardware_cache.rs`: persisted selected hardware/probe-log cache keyed by runtime fingerprint
- `events.rs`: typed broadcast channel plumbing

### Other core files

- `src/config.rs`: TOML config structs, defaults, validation
- `src/orchestrator.rs`: FFmpeg subprocess control and cancellation

## Tech stack

| Layer | Technology |
|------|------------|
| Language | Rust 2024 |
| MSRV | Rust 1.85 |
| Async runtime | Tokio |
| HTTP | Axum 0.7 |
| Database | `sqlx` 0.8 |
| Storage | SQLite WAL |
| Frontend | Astro + React + TypeScript |
| Styling | Tailwind |
| Embedding | `rust-embed` |
