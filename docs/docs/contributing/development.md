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
just dev            # supported on macOS, Linux, and Windows
```

## Common tasks

```bash
just install        # macOS / Linux bootstrap
just install-w      # Windows bootstrap
just check          # supported on macOS, Linux, and Windows
just test           # cargo test
just test-e2e       # Playwright reliability suite
just db-reset       # wipe dev DB, keep config
just db-reset-all   # wipe DB and config (re-triggers wizard)
just bump <version> # bump version in all repo version files
just update <version> # full guarded release flow (Unix-first)
```

## Windows contributor support

Windows contributor support covers the core path:

- `just install-w`
- `just dev`
- `just check`

The following remain Unix-first for now:

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
cargo run           # starts on :3000, or the next free port if 3000 is busy
cargo clippy -- -D warnings
cargo fmt
```

Use `ALCHEMIST_SERVER_PORT=<port> cargo run` to require a
specific port. Startup prints an `INFO` line with the exact
`http://127.0.0.1:<port>` link to open.
