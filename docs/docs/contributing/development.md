---
title: Development Setup
description: Setting up a local Alchemist development environment.
---

## Prerequisites

- Rust 1.85+ (`rustup update stable`)
- [Bun](https://bun.sh/) — frontend package manager
- FFmpeg — required for local testing
- [just](https://github.com/casey/just) — task runner

Node.js is not required. Alchemist uses Bun for all
frontend tooling.

## Clone and run

```bash
git clone https://github.com/bybrooklyn/alchemist.git
cd alchemist
just install        # macOS / Linux bootstrap
just install-w      # Windows bootstrap
just dev            # supported on both paths in RC.2
```

## Common tasks

```bash
just install        # macOS / Linux bootstrap
just install-w      # Windows bootstrap
just check          # supported on both paths in RC.2
just test           # cargo test
just test-e2e       # Playwright reliability suite
just db-reset       # wipe dev DB, keep config
just db-reset-all   # wipe DB and config (re-triggers wizard)
just bump 0.3.0-rc.2 # bump version in all files
just update 0.3.0-rc.2 # full guarded release flow
```

## Windows support in RC.2

Windows contributor support in RC.2 covers the core path:

- `just install-w`
- `just dev`
- `just check`

The following remain Unix-first for now and are deferred to RC.3 or later:

- broader `just` utility recipes such as database and Docker helpers
- release-oriented guarded flows such as `just update`
- full Playwright contributor parity outside the documented manual verification path

## Frontend only

```bash
cd web
bun install --frozen-lockfile
bun run dev         # dev server on :4321
bun run typecheck   # TypeScript check
bun run build       # production build
```

## Backend only

```bash
cargo run           # starts on :3000
cargo clippy -- -D warnings
cargo fmt
```
