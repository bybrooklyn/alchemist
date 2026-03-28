# Alchemist — Instructional Context

This file provides the necessary context for Gemini to understand and work with the Alchemist codebase.

## Project Overview

Alchemist is an automated media library optimization tool written in Rust. It monitors media folders, analyzes files using FFmpeg, and intelligently transcodes them to more efficient formats (AV1, HEVC) when significant space savings are possible without compromising quality.

### Main Technologies
- **Backend:** Rust (Edition 2024), [Axum](https://github.com/tokio-rs/axum) (Web Server), [SQLx](https://github.com/launchbadge/sqlx) (SQLite Database), [Tokio](https://github.com/tokio-rs/tokio) (Asynchronous Runtime).
- **Frontend:** [Astro](https://astro.build/), [React](https://reactjs.org/), [Tailwind CSS](https://tailwindcss.com/), [Lucide React](https://lucide.dev/), [Recharts](https://recharts.org/).
- **Package Manager:** [Bun](https://bun.sh/) (for frontend and docs).
- **Command Runner:** [Just](https://github.com/casey/just).
- **Media Engine:** FFmpeg (external dependency).

### Architecture
- **`src/main.rs`:** Application entry point. Handles CLI arguments, configuration loading, hardware detection, and service initialization.
- **`src/lib.rs`:** Core library exports.
- **`src/media/`:** Core transcoding logic.
    - `planner.rs`: Decisions on whether to transcode.
    - `analyzer.rs`: Extracts media metadata using `ffprobe`.
    - `executor.rs`: Manages `ffmpeg` process execution.
    - `pipeline.rs`: Orchestrates the full transcode lifecycle.
    - `processor.rs`: The `Agent` that runs the main background loop.
- **`src/server/`:** Axum web server implementation, including API routes and SSE (Server-Sent Events) for real-time updates.
- **`src/db.rs`:** SQLite data access layer using SQLx.
- **`migrations/`:** SQL schema migrations.
- **`web/`:** Astro-based frontend dashboard.
- **`docs/`:** Starlight-based documentation site.

## Building and Running

The project uses `just` to simplify common tasks.

### Development
- `just dev`: Starts both the backend (watch mode) and frontend dev server.
- `just run`: Runs the Rust backend directly.
- `just web`: Starts the frontend development server only.
- `just docs-dev`: Starts the documentation development server.

### Build
- `just build`: Performs a full production build (frontend assets + Rust binary).
- `just web-build`: Builds frontend assets only.
- `just rust-build`: Builds the Rust binary only.

### Testing
- `just test`: Runs all Rust tests.
- `just test-e2e`: Runs frontend end-to-end reliability tests.
- `just check`: Runs all linters and typechecks (fmt, clippy, tsc, astro check).

### Database
- `just db-reset`: Wipes the local dev database.
- `just db-reset-all`: Wipes both database and configuration (triggers setup wizard).

## Development Conventions

- **Rust Standards:** Follow standard idiomatic Rust. Use `cargo fmt` and `cargo clippy` (via `just check`).
- **Error Handling:** Use `anyhow` for application-level errors and `thiserror` for library-level errors.
- **Logging:** Use the `tracing` crate for instrumentation.
- **Database:** All schema changes must be implemented as SQL migrations in the `migrations/` directory.
- **Frontend:** Prefer functional React components and Tailwind CSS for styling. Lucide is used for icons.
- **CI/CD:** Github Actions are used for builds, testing, and releases. See `.github/workflows/`.

## Key Files
- `Cargo.toml`: Backend dependencies and metadata.
- `web/package.json`: Frontend dependencies and scripts.
- `justfile`: Command definitions for the project.
- `README.md`: High-level user documentation.
- `CLAUDE.md`: Quick reference for the Claude agent (contains useful build/test commands).
- `DESIGN_PHILOSOPHY.md`: Architectural goals and principles.
- `VERSION`: The current project version.

## Usage Environment Variables
- `ALCHEMIST_CONFIG_PATH`: Path to the `config.toml`.
- `ALCHEMIST_DB_PATH`: Path to the SQLite database.
- `RUST_LOG`: Controls logging verbosity (e.g., `info`, `debug`).
