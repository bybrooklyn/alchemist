# Alchemist Codebase Audit

Alchemist is an intelligent video transcoding automation system written in Rust. It utilizes a background queue with hardware acceleration and CPU fallback to efficiently convert user media libraries to modern codecs (AV1, HEVC, H.264) based on bitrate, resolution, and BPP (bits per pixel) heuristics.

This audit documentation provides a detailed breakdown of the codebase's architecture, core modules, data structures, and the flow of media processing.

## 1. System Architecture & Entry Point

Alchemist can run in two modes:
1. **Server Mode (Default):** Runs an Axum web server providing a REST API and a Web UI. It operates a background loop, monitors watch directories, and processes files according to a schedule.
2. **CLI Mode (`--cli`):** Runs a one-off scan and transcode job over provided directories and exits upon completion.

### `src/main.rs` & `src/lib.rs`
- **Boot Sequence:** Reads command line arguments using `clap`. Initializes the application environment, logging (`tracing`), and database. It dynamically detects hardware (GPU/CPU encoders) and sets up the orchestrator and `Agent` (processor).
- **Configuration Reloading:** Monitors `config.toml` changes using `notify`. On change, it dynamically re-applies configuration, re-detects hardware capabilities, updates concurrent limits, and resets file watchers.

### `src/server.rs`
- The REST API built with `axum`. Serves the frontend (via `rust-embed` or local files) and exposes endpoints to control jobs, retrieve stats, manage configuration, browse the server filesystem (`fs_browser`), and handle authentication (sessions and argon2 password hashing).
- Exposes Server-Sent Events (SSE) at `/api/events` to push realtime logs, progress updates, and job state changes to the web dashboard.
- Utilizes `RateLimitEntry` to manage global and login rate limits.

---

## 2. Core State & Data Management

### `src/db.rs`
- Built on `sqlx` and SQLite (`alchemist.db`), using Write-Ahead Logging (WAL) for concurrency.
- **Job Tracking:** Stores the state (`queued`, `analyzing`, `encoding`, `completed`, `skipped`, `failed`, `cancelled`), retry attempts, priority, progress, and logs for each media file.
- **State Projection:** To ensure robust interaction between the web UI and transcode engine, certain configurations (watch directories, schedule windows, UI preferences, notifications) are "projected" from the central `.toml` config to the database.
- Handles atomic enqueueing of jobs, deduping by file modification time (`mtime_hash`). Includes robust stats aggregation functions (`get_aggregated_stats`, `get_daily_stats`).

### `src/config.rs` & `src/settings.rs`
- Defines the hierarchical structure of `config.toml` (`TranscodeConfig`, `HardwareConfig`, `ScannerConfig`, `ScheduleConfig`, etc.).
- Enums handle configuration mappings like `QualityProfile` (Quality/Balanced/Speed) and map them to their FFmpeg respective CRF/preset flags.
- `settings.rs` manages the logic of hydrating the database with the active state of `config.toml` via `save_config_and_project()`.

---

## 3. Media Pipeline

The processing of media files involves a multi-stage pipeline, orchestrating scanning, probing, decision making, execution, and verification.

### `src/media/pipeline.rs`
This module defines the architectural interfaces and data structures of the entire transcode process.
- **Interfaces:** `Analyzer` (probes the file), `Planner` (decides what to do), `Executor` (runs the transcode).
- **Structures:** `MediaMetadata`, `MediaAnalysis`, `TranscodePlan`, `ExecutionResult`.
- **Pipeline Loop (`Pipeline::process_job`):**
  1. Verifies the input and temp output paths.
  2. Runs the Analyzer.
  3. Runs the Planner. If `TranscodeDecision::Skip` is returned, marks the job as skipped.
  4. Dispatches the TranscodePlan to the Executor.
  5. Computes VMAF scores (if enabled) against the temporary transcoded artifact to ensure quality hasn't drastically degraded.
  6. Promotes the artifact to the final path and updates the database with exact encode sizes and statistics.

### `src/media/processor.rs` (`Agent`)
- Acts as the background task runner. Manages a `tokio::sync::Semaphore` based on the configured concurrency limit.
- Sits in an infinite loop claiming queued jobs from the database and spawning asynchronous tasks to run `Pipeline::process_job`.
- Exposes `pause()`, `resume()`, and `set_scheduler_paused()` hooks to control the global state of the engine.

### `src/media/analyzer.rs` (`FfmpegAnalyzer`)
- Wraps `ffprobe` using a blocking OS process.
- Parses video metadata (duration, FPS, resolution, codec, BPP, 10-bit color, HDR transfer functions like PQ/HLG) and outputs an `AnalysisConfidence` (High, Medium, Low) based on how complete the metadata is.

### `src/media/planner.rs` (`BasicPlanner`)
The intelligence layer of Alchemist.
- **Decision Engine (`should_transcode`):** Skips files that are already the target codec and 10-bit. Calculates the Bits-Per-Pixel (BPP) and normalizes it based on resolution. If the BPP is lower than the quality threshold (to avoid generational quality loss), or if the file is smaller than `min_file_size_mb`, it skips the file.
- **Encoder Selection (`select_encoder`):** Evaluates `HardwareInfo` (available hardware backends) against the requested output codec. It prefers GPU encoders (NVENC, QSV, VAAPI, AMF, VideoToolbox) and falls back to CPU encoders (SVT-AV1, libx265) only if configured.
- **Subtitles & Audio:** Determines if audio can be copied or must be transcoded (Opus/AAC) based on container compatibility (e.g. mp4 vs mkv) and "heavy" codecs (TrueHD/FLAC). Plans subtitle burn-ins or sidecar extraction (`.mks`).

### `src/media/executor.rs` (`FfmpegExecutor`)
- Implements the `Executor` trait. Links `TranscodePlan` to the `Transcoder`.
- Provides an implementation of `ExecutionObserver` which listens to standard error outputs from FFmpeg to persist textual logs and calculated percentage progress to the database and SSE broadcast channel.
- Runs a post-transcode probe on the output to ensure the actual executed codec and hardware tags match the requested plan (detecting transparent failures).

---

## 4. Execution & Orchestration

### `src/orchestrator.rs` (`Transcoder`)
- A robust, low-level wrapper around the `ffmpeg` subprocess.
- Manages an internal state of `cancel_channels` and `pending_cancels` (`HashMap<i64, oneshot::Sender<()>>`). If a job is cancelled via the UI, it sends a kill signal to the exact tokio process.
- Streams FFmpeg output to observers line-by-line while simultaneously detecting crashes, emitting `AlchemistError::FFmpeg` with the last 20 lines of standard error context if the process fails.

---

## 5. System Components

### `src/system/hardware.rs`
- Automatically probes the host for GPU acceleration capabilities.
- Determines the active vendor (Nvidia, AMD, Intel, Apple, CPU).
- Executes dummy FFmpeg `lavfi` (black frame) encode tests against known hardware encoder strings (e.g. `hevc_vaapi`, `av1_qsv`, `h264_nvenc`) to empirically verify that the system environment/drivers are correctly configured before claiming an encoder is available.
- Handles explicit `/dev/dri/renderD128` overrides for Linux Docker containers.

### `src/media/scanner.rs`
- Utilizes `rayon` for fast, parallel recursive file-system scanning.
- Filters target directories based on user-defined file extensions. Returns a sorted list of `DiscoveredMedia` ready for DB ingestion.

### `src/scheduler.rs`
- Checks an array of configured `ScheduleWindow` records from the DB every 60 seconds.
- Calculates if the current minute of the current day lies within an active window.
- Dynamically invokes `agent.set_scheduler_paused()` to restrict the CPU/GPU workload outside of allowed server hours.

---

## Summary

Alchemist combines a highly concurrent Rust backend (`tokio`, `axum`) with empirical validation mechanisms (VMAF scoring, FFmpeg test probes). Its architecture heavily isolates **Planning** (heuristic decision logic) from **Execution** (running the transcode), ensuring that the system can gracefully fall back, test different hardware topologies, and avoid re-transcoding media without degrading video quality.