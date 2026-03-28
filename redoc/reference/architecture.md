# Architecture

How Alchemist is structured internally.

## Pipeline overview

Every file Alchemist processes goes through this sequence:

```text
Scanner -> jobs table (queued)
    ↓
Agent::run_loop() - picks next job, acquires semaphore permit
    ↓
FfmpegAnalyzer::analyze() - ffprobe -> MediaMetadata
    ↓
BasicPlanner::plan() - should_transcode() -> Skip / Remux / Transcode
    ↓
Skip       -> decision stored, done
Remux      -> ffmpeg -c copy (MP4->MKV, lossless)
Transcode  -> FfmpegExecutor::execute()
    ↓
Post-encode: optional VMAF, promote temp file, update stats
    ↓
DB updated: completed / failed / skipped
```

## Source layout

```text
src/
├── main.rs               Boot, CLI args, config reload watcher
├── config.rs             TOML config structs and validation
├── db.rs                 SQLite - all queries (~2400 lines)
├── orchestrator.rs       FFmpeg subprocess wrapper
├── scheduler.rs          Schedule window enforcement
├── notifications.rs      Discord, Gotify, webhook dispatch
├── media/
│   ├── pipeline.rs       Core interfaces, TranscodePlan structs
│   ├── planner.rs        Decision engine (BasicPlanner)
│   ├── analyzer.rs       ffprobe wrapper with 120s timeout
│   ├── executor.rs       Plan -> FFmpeg execution
│   ├── processor.rs      Agent - job loop + engine state machine
│   ├── scanner.rs        Filesystem walker
│   ├── health.rs         Library Doctor health checker
│   └── ffmpeg/           FFmpeg arg builders per encoder type
│       ├── mod.rs        Main builder
│       ├── vaapi.rs      Intel/AMD Linux
│       ├── qsv.rs        Intel QSV (deprecated for Arc)
│       ├── nvenc.rs      NVIDIA
│       ├── amf.rs        AMD Windows
│       ├── videotoolbox.rs  Apple Silicon
│       └── cpu.rs        SVT-AV1, x265, x264
└── server/               HTTP layer (10 modules)
    ├── mod.rs            AppState, run_server, route table
    ├── auth.rs           Login, logout, sessions (Argon2)
    ├── jobs.rs           Job queue API
    ├── scan.rs           Library scan endpoints
    ├── settings.rs       Config read/write
    ├── stats.rs          Aggregate stats and savings
    ├── system.rs         Hardware detection, resource monitor
    ├── sse.rs            Server-Sent Events stream
    ├── middleware.rs     Rate limiting, auth middleware
    └── wizard.rs         First-run setup API
```

## Engine state machine

The `Agent` in `src/media/processor.rs` controls when jobs run:

| State | Behavior |
|-------|----------|
| Running | Normal - jobs start as permits become available |
| Paused | No new jobs start; active jobs freeze mid-encode |
| Draining | No new jobs start; active jobs finish normally |
| Scheduler paused | Same as Paused but triggered by schedule windows |

Engine always starts paused on boot. Users must explicitly
click **Start**.

## Engine modes

Engine modes auto-compute the concurrent job limit from
CPU count:

| Mode | Concurrent jobs |
|------|----------------|
| Background | 1 |
| Balanced | floor(cpu_count / 2), min 1, max 4 (default) |
| Throughput | floor(cpu_count / 2), min 1, uncapped |

Users can override the computed limit via Settings -> Runtime.

## Technology stack

| Layer | Technology |
|-------|------------|
| Runtime | Rust 2024 edition, Tokio, MSRV 1.85 |
| Web framework | Axum 0.7 |
| Database | SQLite via sqlx 0.8, WAL mode |
| Frontend | Astro + React + TypeScript + Tailwind CSS |
| Build | Bun (frontend), Cargo (Rust) |
| Media | FFmpeg (external) + FFprobe |
| Packaging | rust-embed (single binary) |
