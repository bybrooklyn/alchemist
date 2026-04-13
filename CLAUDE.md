# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Project Is

Alchemist is a self-hosted media transcoding pipeline. It scans a media library, analyzes video files for transcoding opportunities, and intelligently encodes them using hardware acceleration (NVIDIA NVENC, Intel QSV, AMD VAAPI/AMF, Apple VideoToolbox) with CPU fallback. It includes a web UI for configuration and monitoring.

**Stack:** Rust (Axum + SQLite/sqlx + tokio) backend, Astro 5 + React 18 + TypeScript frontend.

## Commands

All common tasks are in the `justfile` — use `just` as the task runner.

### Development
```bash
just dev          # Build frontend assets, then start the backend
just run          # Backend only
just web          # Frontend dev server only
```

### Build
```bash
just build        # Full production build (frontend first, then Rust binary)
just web-build    # Frontend assets only
just rust-build   # Rust binary only (assumes web/dist exists)
```

### Checks & Linting (mirrors CI exactly)
```bash
just check        # All checks: fmt + clippy + typecheck + frontend build
just check-rust   # Rust only (faster)
just check-web    # Frontend only
```

### Tests
```bash
just test                      # All Rust tests
just test-filter <pattern>     # Single test by name (e.g., just test-filter stream_rules)
just test-verbose              # All tests with stdout visible
just test-e2e                  # Playwright e2e tests (headless)
just test-e2e-headed           # E2e with browser visible
```

Integration tests require FFmpeg and FFprobe installed locally.

Integration tests live in `tests/` — notably `integration_db_upgrade.rs` tests schema migrations against a v0.2.5 baseline database. Every migration must pass this.

### Database
```bash
just db-reset       # Wipe dev database (keeps config)
just db-reset-all   # Wipe database AND config (triggers setup wizard on next run)
just db-shell       # SQLite shell
```

## Architecture

### Clippy Strictness

CI enforces `-D clippy::unwrap_used` and `-D clippy::expect_used`. Use `?` propagation or explicit match — no `.unwrap()` or `.expect()` in production code paths.

### Rust Backend (`src/`)

The backend is structured around a central `AppState` (holding SQLite pool, config, broadcast channels) passed to Axum handlers:

- **`server/`** — HTTP layer split into focused modules:
  - `mod.rs` — `AppState`, `run_server`, route registration, and static asset serving
  - `auth.rs` — Login, logout, session management (Argon2)
  - `jobs.rs` — Job queue API: list, detail, cancel, restart, priority, batch operations
  - `scan.rs` — Library scan trigger and status endpoints
  - `settings.rs` — All config read/write endpoints
  - `stats.rs` — Aggregate stats, savings, and daily history
  - `system.rs` — Hardware detection, resource monitor, library health
  - `sse.rs` — Server-Sent Events stream
  - `middleware.rs` — Rate limiting and auth middleware
  - `wizard.rs` — First-run setup API endpoints
- **`db.rs`** (~2400 LOC) — SQLite connection pool, all queries, migration runner. Direct sqlx usage; no ORM.
- **`config.rs`** (~850 LOC) — TOML config structs for all user-facing settings.
- **`media/`** — The core pipeline:
  - `scanner.rs` — File discovery (glob patterns, exclusion rules)
  - `analyzer.rs` — FFprobe-based stream inspection
  - `planner.rs` — Decision logic for whether/how to transcode
  - `pipeline.rs` — Orchestrates scan → analyze → plan → execute
  - `processor.rs` — Job queue controller (concurrency, pausing, draining)
  - `ffmpeg/` — FFmpeg command builder and progress parser, with platform-specific encoder modules
- **`orchestrator.rs`** — Spawns and monitors FFmpeg processes, streams progress back via channels. Uses `std::sync::Mutex` (not tokio) intentionally — critical sections never cross `.await` boundaries.
- **`system/`** — Hardware detection (`hardware.rs`), file watcher (`watcher.rs`), library scanner (`scanner.rs`)
- **`scheduler.rs`** — Off-peak cron scheduling
- **`notifications.rs`** — Discord, Gotify, Webhook integrations
- **`wizard.rs`** — First-run setup flow

#### Event Channel Architecture

Three typed broadcast channels in `AppState` (defined in `db.rs`):
- `jobs` (capacity 1000) — high-frequency: progress, state changes, decisions, logs
- `config` (capacity 50) — watch folder changes, settings updates
- `system` (capacity 100) — scan lifecycle, hardware state changes

`sse.rs` merges all three via `futures::stream::select_all`. SSE is capped at 50 concurrent connections (`MAX_SSE_CONNECTIONS`), enforced with a RAII guard that decrements on stream drop.

`AlchemistEvent` still exists as a legacy bridge; `JobEvent` is the canonical type — new code uses `JobEvent`/`ConfigEvent`/`SystemEvent`.

#### FFmpeg Command Builder

`FFmpegCommandBuilder<'a>` in `src/media/ffmpeg/mod.rs` uses lifetime references to avoid cloning input/output paths. `.with_hardware(Option<&HardwareInfo>)` injects hardware flags; `.build_args()` returns `Vec<String>` for unit testing without spawning a process. Each hardware platform is a submodule (amf, cpu, nvenc, qsv, vaapi, videotoolbox). `EncoderCapabilities` is detected once via live ffmpeg invocation and cached in `OnceLock`.

### Frontend (`web/src/`)

Astro pages (`web/src/pages/`) with React islands. UI reflects backend state via SSE — avoid optimistic UI unless reconciled with backend truth.

Job management UI is split into focused subcomponents under `web/src/components/jobs/`: `JobsTable`, `JobDetailModal`, `JobsToolbar`, `JobExplanations`, `useJobSSE.ts` (SSE hook), and `types.ts` (shared types + pure data utilities). `JobManager.tsx` is the parent that owns state and wires them together.

### Database Schema

Migrations in `migrations/` are **additive only** — never rename or drop columns. Databases from v0.2.5+ must remain usable. When adding schema: add columns with defaults or nullable, or add new tables.

## Key Design Constraints

From `DESIGN_PHILOSOPHY.md` — these are binding:

- **Never overwrite user media by default.** Always prefer reversible actions.
- **Backwards compatibility:** DBs from v0.2.5+ must work with all future versions.
- **Schema changes are additive only** — no renames, no drops.
- **No data loss on failure** — fail safe, not fail open.
- **All core features must work on macOS, Linux, and Windows.**
- **Deterministic behavior** — no clever heuristics; explicit error handling over implicit fallbacks.
- If a change risks data loss or breaks older data: do not merge it.

## Environment Variables

```
ALCHEMIST_CONFIG_PATH   # Config file path (default: ~/.config/alchemist/config.toml)
ALCHEMIST_DB_PATH       # Database path (default: ~/.config/alchemist/alchemist.db)
ALCHEMIST_CONFIG_MUTABLE # Allow runtime config changes (default: true)
RUST_LOG                # Log level (e.g., info, alchemist=debug)
```

## Release Process

```bash
just update <VERSION>   # Validates, runs tests, bumps version everywhere, commits, tags, pushes
```

CI runs on GitHub Actions: `rust-check`, `rust-test`, `frontend-check` (see `.github/workflows/ci.yml`). Releases build for Linux x86_64/ARM64, Windows x86_64, macOS Intel/Apple Silicon, and Docker (linux/amd64 + linux/arm64).
