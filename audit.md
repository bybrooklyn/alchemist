# Alchemist Codebase Audit

Alchemist is an intelligent video transcoding automation system written in Rust. It maintains a background queue with hardware acceleration and CPU fallback to convert media libraries to modern codecs (AV1, HEVC, H.264) based on bitrate, resolution, and BPP (bits per pixel) heuristics.

*Last updated: v0.3.0-rc.3*

## 1. System Architecture & Entry Point

Alchemist can run in two modes:
1. **Server Mode (Default):** Runs an Axum web server providing a REST API and web UI. Monitors watch directories, processes jobs according to schedule, and emits real-time events via SSE.
2. **CLI Mode (`--cli`):** Runs a one-off scan and transcode job over provided directories and exits.

### `src/main.rs` & `src/lib.rs`
- **Boot Sequence:** Parses CLI args (`clap`), initializes logging (`tracing`), runs database migrations, performs hardware detection, and starts the orchestrator and `Agent`.
- **Configuration Reloading:** Monitors `config.toml` via `notify`. On change, re-applies config, re-detects hardware, updates concurrency limits, and resets file watchers.

### `src/server.rs`
- REST API built with Axum (~4700 LOC). Serves the frontend (embedded via `rust-embed` with the `embed-web` feature, or from `web/dist/` on disk otherwise) and handles authentication (Argon2 password hashing, session tokens), rate limiting, SSE events, filesystem browsing, and all API endpoints.
- **SSE at `/api/events`:** pushes real-time logs, progress updates, and job state changes to the dashboard.
- **Engine runtime modes:** `background`, `balanced`, and `throughput` — exposed through the API and dashboard header, with manual concurrency overrides and drain/resume controls.

---

## 2. Core State & Data Management

### `src/db.rs`
- SQLite with `sqlx` and WAL mode for concurrent reads.
- **Job states:** `queued → analyzing → encoding → completed | skipped | failed | cancelled`.
- **Retry backoff:** Failed jobs re-enter `queued` with exponential delay — attempt 1: +5 min, attempt 2: +15 min, attempt 3: +60 min, attempt 4+: +360 min. Implemented directly in SQL (`CASE WHEN attempt_count = N THEN datetime(updated_at, '+N minutes')`).
- **Deduplication:** Jobs are keyed on `input_path` + `mtime_hash` to prevent re-enqueueing unchanged files.
- **State projection:** Watch directories, schedule windows, UI preferences, and notifications are "projected" from `config.toml` into the database so the web UI and engine share a single source of truth.
- **Log retention:** Configurable pruning (default 30 days) prevents unbounded growth.
- **Session cleanup:** Expired auth sessions are pruned at startup and every 24 hours.

### `src/config.rs` & `src/settings.rs`
- Defines the hierarchical `config.toml` structure (`TranscodeConfig`, `HardwareConfig`, `ScannerConfig`, `QualityConfig`, `ScheduleConfig`, `FilesConfig`, `StreamRules`, etc.).
- `QualityProfile` (Quality/Balanced/Speed) maps to FFmpeg CRF and preset flags.
- **Stream rules** (`StreamRules`): strip audio tracks by title keyword (e.g. commentary), filter by language (`keep_audio_languages`), keep only the default audio track.
- **Per-library profiles** (`BuiltInLibraryProfile`): four built-in presets (Space Saver, Quality First, Balanced, Streaming) that each watch folder can override globally.
- `settings.rs::save_config_and_project()` atomically persists config to disk and projects it to the database.

---

## 3. Media Pipeline

### `src/media/pipeline.rs`
Defines the `Analyzer`, `Planner`, and `Executor` interfaces and the `Pipeline::process_job` loop:
1. Verifies input path and reserves a temp output path.
2. Runs the `Analyzer` (FFprobe).
3. Runs the `Planner`. Returns `TranscodeDecision::Skip` → job marked skipped.
4. Dispatches the `TranscodePlan` to the `Executor` (FFmpeg).
5. Optionally computes VMAF score against the temp artifact. If below `min_vmaf_score`, the encode is rejected and the job fails (no silent quality loss).
6. Promotes the artifact to the final path, records output size and savings statistics.

### `src/media/processor.rs` (`Agent`)
- Background task runner managing a `tokio::sync::Semaphore` for concurrency.
- Claims queued jobs from the database (respecting retry backoff) and spawns async tasks via `Pipeline::process_job`.
- Exposes `pause()`, `resume()`, `drain()`, and `set_scheduler_paused()` for engine control.

### `src/media/analyzer.rs` (`FfmpegAnalyzer`)
- Wraps `ffprobe` as a blocking subprocess.
- Parses video metadata: duration, FPS, resolution, codec, BPP, 10-bit color, HDR transfer functions (PQ/HLG).
- Emits `AnalysisConfidence` (High/Medium/Low) based on metadata completeness.

### `src/media/planner.rs` (`BasicPlanner`)
The decision layer:
- **Skip conditions:** already target codec and 10-bit; BPP below quality threshold (avoids generational loss); file smaller than `min_file_size_mb`; matches stream rules exclusion.
- **Encoder selection:** prefers GPU (NVENC → QSV → VAAPI/AMF → VideoToolbox) then falls back to CPU (SVT-AV1, libx265, libx264) only if configured.
- **Remux planning:** detects cases where only a container change is needed (no re-encode).
- **Audio:** copy or transcode to Opus/AAC based on container compatibility and heavy codec detection (TrueHD/FLAC). Stream rules applied here.
- **Subtitles:** burn-in or sidecar extraction (`.mks`).

### `src/media/executor.rs` (`FfmpegExecutor`)
- Connects `TranscodePlan` to the `Transcoder`.
- `ExecutionObserver` streams FFmpeg stderr line-by-line, persisting logs and progress % to the database and SSE channel.
- Post-transcode probe verifies the output codec and hardware tags match the plan (catches transparent failures).

### `src/media/health.rs` (`HealthChecker`)
- Library Doctor: runs per-file health checks (probes for corrupt/truncated files) on demand from System Settings.
- Results recorded via `db.record_health_check()`; runs tracked with `create_health_scan_run()` / `complete_health_scan_run()`.

---

## 4. Execution & Orchestration

### `src/orchestrator.rs` (`Transcoder`)
- Low-level `ffmpeg` subprocess wrapper.
- `cancel_channels: HashMap<i64, oneshot::Sender<()>>` — cancellation sends a kill signal to the exact process.
- On FFmpeg crash, emits `AlchemistError::FFmpeg` with the last 20 lines of stderr for diagnostics.

---

## 5. System Components

### `src/system/hardware.rs`
- Probes for GPU acceleration at startup by running dummy `lavfi` (black frame) encodes against known encoder strings (`hevc_vaapi`, `av1_qsv`, `h264_nvenc`, etc.).
- Handles `/dev/dri/renderD128` overrides for Linux Docker containers.
- Results cached in `HardwareState` and re-probed on config reload.

### `src/media/scanner.rs`
- `rayon`-parallel recursive filesystem scan.
- Filters by configured extensions; returns sorted `DiscoveredMedia` for DB ingestion.

### `src/system/watcher.rs`
- Uses the `notify` crate to watch configured directories for new files, triggering immediate enqueue without waiting for a scheduled scan.

### `src/scheduler.rs`
- Polls configured `ScheduleWindow` records every 60 seconds.
- Calls `agent.set_scheduler_paused()` to halt processing outside allowed hours.

### `src/notifications.rs`
- Discord webhook, Gotify, and generic webhook integration.
- Triggered on job completion/failure events.

---

## 6. Statistics & Reporting

### Storage savings dashboard
- `/api/stats` aggregates total space recovered, average reduction %, per-codec breakdowns, and daily savings over time via `db.get_aggregated_stats()` / `db.get_daily_stats()`.
- Data displayed on the Stats page with charts (Recharts).

---

## Summary

Alchemist isolates **Planning** (heuristic decision logic) from **Execution** (FFmpeg subprocess management), with empirical validation at each boundary — hardware probes confirm encoders work before committing to them, VMAF scoring rejects encodes that degrade quality, and a post-transcode codec probe catches silent failures. The retry backoff, orphan cleanup on startup, and additive-only schema migrations reflect a reliability-first design philosophy.
