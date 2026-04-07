# Alchemist Context

Alchemist is a media transcoding tool designed for simplicity and space-saving. It provides a web-based dashboard to manage media libraries, monitor transcoding progress, and configure encoding profiles with automatic hardware acceleration support.

## Project Overview

- **Purpose:** Automated media transcoding and library management to save storage space.
- **Backend:** Rust 2024 (Tokio, Axum 0.7, SQLx 0.8 with SQLite).
- **Frontend:** Astro 5, React 18, Tailwind CSS (embedded in the Rust binary via `rust-embed`).
- **Documentation:** Docusaurus 3.9 (located in `docs/package.json`).
- **Core Technologies:** FFmpeg (analyzer and executor), SQLite (state management), Argon2 (security).

## Architecture & Pipeline

The core logic is managed by an `Agent` in `src/media/processor.rs` that orchestrates the following pipeline:
1. **Scanner:** Finds files in watch folders and enqueues jobs.
2. **Analyzer:** Runs `ffprobe` to build normalized media metadata.
3. **Planner:** Decides whether to skip, remux, or transcode based on quality profiles.
4. **Executor:** Runs `ffmpeg` with selected encoders (NVIDIA, Intel, AMD, Apple, or CPU fallback).
5. **Post-Encode:** Quality checks (optional VMAF), file promotion, and database updates.

### Engine States & Modes
- **States:** `Running`, `Paused`, `Draining`, `SchedulerPaused`.
- **Modes:** `Background` (1 job), `Balanced` (capped concurrency), `Throughput` (uncapped concurrency).

## Building and Running

The project uses `just` as the primary task runner.

- **Development:**
  - `just dev`: Builds frontend assets and starts the backend server.
  - `just web`: Starts the frontend development server (Astro) independently.
  - `just run`: Starts the backend only (requires `web/dist` to exist).
- **Build:**
  - `just build`: Performs a full production build (Frontend then Rust).
  - `just docker-build`: Builds the local Docker image.
- **Verification:**
  - `just release-check`: Runs all checks (Rust fmt, clippy, check; Frontend typecheck, build).
  - `just check-rust`: Rust-only verification.
- **Testing:**
  - `just test`: Runs all Rust tests.
  - `just test-e2e`: Runs Playwright reliability tests in `web-e2e/`.

## Development Conventions

- **MSRV:** Rust 1.85 (Rust 2024 edition).
- **Task Management:** Always check the `justfile` for established workflows before adding new scripts.
- **Configuration:** Respects `ALCHEMIST_CONFIG_PATH` and `ALCHEMIST_DB_PATH`. Hot-reloading is supported for the configuration file.
- **Database:** Uses SQLx migrations located in `migrations/`. Use `just db-reset` to wipe the local database for testing.
- **Code Style:** Standard Rust formatting (`cargo fmt`) and clippy (`cargo clippy`) are enforced in CI.
- **Documentation:** Maintain documentation in `docs/` (Markdown) and the Docusaurus site.

## Key Directories

- `src/`: Backend source code.
  - `server/`: Axum API routes and handlers.
  - `media/`: Transcoding pipeline, planners, and processors.
  - `db/`: SQLx database access layer.
- `web/`: Frontend source code (Astro + React).
- `docs/`: Comprehensive technical documentation.
- `migrations/`: SQLite database migrations.
- `tests/`: Integration and unit tests for the backend.
- `web-e2e/`: Playwright end-to-end tests for the web interface.
