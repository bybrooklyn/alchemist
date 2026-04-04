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
just install        # macOS / Linux
just install-w      # Windows
just dev   # build frontend assets, then start the backend
```

## Common tasks

```bash
just install        # macOS / Linux bootstrap
just install-w      # Windows bootstrap
just check          # fmt + clippy + typecheck + build (mirrors CI)
just test           # cargo test
just test-e2e       # Playwright reliability suite
just db-reset       # wipe dev DB, keep config
just db-reset-all   # wipe DB and config (re-triggers wizard)
just bump 0.3.0     # bump version in all files
just update 0.3.0   # full guarded release flow
```

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
