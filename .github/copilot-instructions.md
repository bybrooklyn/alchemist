# Copilot Instructions

Alchemist is a self-hosted media transcoding pipeline. It scans media libraries, analyzes video files, and intelligently encodes them using hardware acceleration (NVIDIA NVENC, Intel QSV, AMD VAAPI/AMF, Apple VideoToolbox) with CPU fallback.

**Stack:** Rust (Axum + SQLite/sqlx + tokio) backend, Astro 5 + React 18 + TypeScript frontend, Bun package manager.

## Commands

All tasks use `just` (install: `cargo install just`).

```bash
# Development
just dev              # Backend (watch) + frontend dev server
just run              # Backend only
just web              # Frontend only

# Build
just build            # Full production build
just check            # All checks (mirrors CI): fmt + clippy + typecheck + build

# Tests
just test                       # All Rust tests
just test-filter <pattern>      # Single test (e.g., just test-filter stream_rules)
just test-e2e                   # Playwright e2e tests

# Database
just db-reset         # Wipe dev database
just db-reset-all     # Wipe database AND config (triggers setup wizard)
```

Integration tests require FFmpeg/FFprobe installed locally.

## Architecture

### Backend (`src/`)

Central `AppState` holds SQLite pool, config, and broadcast channels, passed to Axum handlers.

- **`server/`** — HTTP routes, SSE events, auth (Argon2 + sessions), rate limiting, static assets
- **`db.rs`** — All SQLite queries via sqlx (no ORM), migration runner
- **`config.rs`** — TOML config structs
- **`media/`** — Core pipeline:
  - `scanner.rs` → `analyzer.rs` → `planner.rs` → `pipeline.rs` → `processor.rs`
  - `ffmpeg/` — Command builder with platform-specific encoder modules
- **`orchestrator.rs`** — FFmpeg process spawning and progress streaming
- **`system/`** — Hardware detection, file watcher, library scanner
- **`scheduler.rs`** — Off-peak cron scheduling
- **`notifications.rs`** — Discord, Gotify, Webhook integrations

### Frontend (`web/src/`)

Astro pages with React islands. UI reflects backend state via SSE — avoid optimistic UI unless reconciled with backend truth.

## Key Constraints

These are binding project rules:

- **Never overwrite user media by default** — always prefer reversible actions
- **Schema changes are additive only** — no renames, no drops; DBs from v0.2.5+ must remain usable
- **Cross-platform** — all core features must work on macOS, Linux, and Windows
- **Fail safe** — no data loss on failure; explicit error handling over implicit fallbacks

## Conventions

- **Error handling:** `anyhow` for application errors, `thiserror` for library errors
- **Logging:** `tracing` crate
- **Frontend icons:** `lucide-react`
- **Animations:** `framer-motion`
- **API calls:** Custom `apiFetch` utility from `web/src/lib/api`
- **Promise handling in React:** Use `void` prefix for async calls in useEffect

## Environment Variables

```
ALCHEMIST_CONFIG_PATH   # Config file (default: ~/.config/alchemist/config.toml)
ALCHEMIST_DB_PATH       # Database (default: ~/.config/alchemist/alchemist.db)
RUST_LOG                # Log level (e.g., info, alchemist=debug)
```

## MCP Servers

For browser automation and e2e test development, add the Playwright MCP server to your configuration:

```json
{
  "mcpServers": {
    "playwright": {
      "command": "npx",
      "args": ["@playwright/mcp@latest"]
    }
  }
}
```

This enables Copilot to interact with the browser for debugging UI issues and developing e2e tests in `web-e2e/`.
