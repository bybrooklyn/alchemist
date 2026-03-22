# Alchemist — Justfile
# https://github.com/casey/just
#
# Install: cargo install just  |  brew install just  |  pacman -S just

set shell := ["bash", "-euo", "pipefail", "-c"]

# ─────────────────────────────────────────
# Variables
# ─────────────────────────────────────────

VERSION := `cat VERSION | tr -d '[:space:]'`

# ─────────────────────────────────────────
# Default — list all recipes
# ─────────────────────────────────────────

[private]
default:
    @just --list

# ─────────────────────────────────────────
# DEVELOPMENT
# ─────────────────────────────────────────

# Start backend (watch mode) + frontend dev server concurrently
dev:
    @echo "Starting Alchemist dev servers..."
    @trap 'kill 0' INT; \
        (cd web && bun run dev) & \
        cargo watch -x run & \
        wait

# Start the backend only
run:
    cargo run

# Start frontend dev server only
web:
    cd web && bun install --frozen-lockfile && bun run dev

# ─────────────────────────────────────────
# BUILD
# ─────────────────────────────────────────

# Full production build — frontend first, then Rust
build:
    @echo "Building frontend..."
    cd web && bun install --frozen-lockfile && bun run build
    @echo "Building Rust binary..."
    cargo build --release
    @echo "Done → target/release/alchemist"

# Build frontend assets only
web-build:
    cd web && bun install --frozen-lockfile && bun run build

# Build Rust only (assumes web/dist already exists)
rust-build:
    cargo build --release

# ─────────────────────────────────────────
# CHECKS — mirrors CI exactly
# ─────────────────────────────────────────

# Run all checks (fmt + clippy + typecheck + frontend build)
check:
    @echo "── Rust format ──"
    cargo fmt --all -- --check
    @echo "── Rust clippy ──"
    cargo clippy --all-targets --all-features -- -D warnings
    @echo "── Rust check ──"
    cargo check --all-targets
    @echo "── Frontend typecheck ──"
    cd web && bun install --frozen-lockfile && bun run typecheck
    @echo "── Frontend build ──"
    cd web && bun run build
    @echo "All checks passed ✓"

# Rust checks only (faster)
check-rust:
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo check --all-targets

# Frontend checks only
check-web:
    cd web && bun install --frozen-lockfile && bun run typecheck && bun run build

# ─────────────────────────────────────────
# TESTS
# ─────────────────────────────────────────

# Run all Rust tests
test:
    cargo test

# Run Rust tests with output shown
test-verbose:
    cargo test -- --nocapture

# Run a specific test by name (e.g. just test-filter stream_rules)
test-filter FILTER:
    cargo test {{FILTER}} -- --nocapture

# Run frontend e2e reliability tests
test-e2e:
    cd web-e2e && bun install --frozen-lockfile && bun run test:reliability

# Run all e2e tests headed (for debugging)
test-e2e-headed:
    cd web-e2e && bun install --frozen-lockfile && bun run test:headed

# Run all e2e tests with Playwright UI
test-e2e-ui:
    cd web-e2e && bun install --frozen-lockfile && bun run test:ui

# ─────────────────────────────────────────
# DATABASE
# ─────────────────────────────────────────

# Wipe the dev database (essential for re-testing the setup wizard)
db-reset:
    @DB="${ALCHEMIST_DB_PATH:-${XDG_CONFIG_HOME:-$HOME/.config}/alchemist/alchemist.db}"; \
        echo "Deleting $DB"; \
        rm -f "$DB"; \
        echo "Done — next run will re-apply migrations."

# Wipe dev database AND config (full clean slate, triggers setup wizard)
db-reset-all:
    @DB="${ALCHEMIST_DB_PATH:-${XDG_CONFIG_HOME:-$HOME/.config}/alchemist/alchemist.db}"; \
        CFG="${ALCHEMIST_CONFIG_PATH:-${XDG_CONFIG_HOME:-$HOME/.config}/alchemist/config.toml}"; \
        echo "Deleting $DB and $CFG"; \
        rm -f "$DB" "$CFG"; \
        echo "Done — setup wizard will run on next launch."

# Open the dev database in sqlite3
db-shell:
    @sqlite3 "${ALCHEMIST_DB_PATH:-${XDG_CONFIG_HOME:-$HOME/.config}/alchemist/alchemist.db}"

# Show applied migrations
db-migrations:
    @sqlite3 "${ALCHEMIST_DB_PATH:-${XDG_CONFIG_HOME:-$HOME/.config}/alchemist/alchemist.db}" \
        "SELECT version, description, installed_on FROM _sqlx_migrations ORDER BY installed_on;"

# ─────────────────────────────────────────
# DOCKER
# ─────────────────────────────────────────

# Build the Docker image locally
docker-build:
    docker build -t alchemist:dev .

# Build multi-arch image (requires buildx)
docker-build-multi:
    docker buildx build --platform linux/amd64,linux/arm64 -t alchemist:dev .

# Run Alchemist in Docker for local testing
docker-run:
    docker run --rm \
        -p 3000:3000 \
        -v "$(pwd)/dev-data:/app/data" \
        -e ALCHEMIST_DB_PATH=/app/data/alchemist.db \
        -e ALCHEMIST_CONFIG_MUTABLE=true \
        -e RUST_LOG=info \
        alchemist:dev

# Start the Docker Compose stack
docker-up:
    docker compose up -d

# Stop the Docker Compose stack
docker-down:
    docker compose down

# Tail Docker Compose logs
docker-logs:
    docker compose logs -f

# ─────────────────────────────────────────
# VERSIONING & RELEASE
# ─────────────────────────────────────────

# Bump version across all files (e.g. just bump 0.3.0 or just bump 0.3.0-rc.1)
bump NEW_VERSION:
    @echo "Bumping to {{NEW_VERSION}}..."
    bash scripts/bump_version.sh {{NEW_VERSION}}

# Show current version
version:
    @echo "{{VERSION}}"

# Open CHANGELOG.md
changelog:
    ${EDITOR:-vi} CHANGELOG.md

# Print a pre-filled changelog entry header for pasting
changelog-entry:
    @printf '\n## [v{{VERSION}}] - %s\n\n### Added\n- \n\n### Changed\n- \n\n### Fixed\n- \n\n' \
        "$(date +%Y-%m-%d)"

# Run all checks and tests, then print release steps
release-check:
    @echo "── Release checklist for v{{VERSION}} ──"
    @just check
    @just test
    @echo ""
    @echo "✓ All checks passed. Next steps:"
    @echo "  1. Update CHANGELOG.md"
    @echo "  2. git commit -am 'chore: release v{{VERSION}}'"
    @echo "  3. git tag -a v{{VERSION}} -m 'v{{VERSION}}'"
    @echo "  4. git push && git push --tags"

# ─────────────────────────────────────────
# DOCS SITE
# ─────────────────────────────────────────

# Start the docs dev server
docs-dev:
    cd docs && bun install && bun run dev

# Build the docs site
docs-build:
    cd docs && bun install --frozen-lockfile && bun run build

# ─────────────────────────────────────────
# UTILITIES
# ─────────────────────────────────────────

# Format all Rust code
fmt:
    cargo fmt --all

# Clean all build artifacts
clean:
    cargo clean
    rm -rf web/dist web/node_modules web-e2e/node_modules docs/node_modules

# Count lines of source code
loc:
    @echo "── Rust ──"
    @find src -name "*.rs" | xargs wc -l | tail -1
    @echo "── Frontend ──"
    @find web/src -name "*.ts" -o -name "*.tsx" -o -name "*.astro" \
        | xargs wc -l | tail -1
    @echo "── Tests ──"
    @find tests web-e2e/tests -name "*.rs" -o -name "*.ts" \
        2>/dev/null | xargs wc -l | tail -1

# Show all environment variables Alchemist respects
env-help:
    @echo "ALCHEMIST_CONFIG_PATH    Config file path"
    @echo "                         Linux/macOS default: ~/.config/alchemist/config.toml"
    @echo "                         Windows default:     %APPDATA%\\Alchemist\\config.toml"
    @echo "ALCHEMIST_CONFIG         Alias for ALCHEMIST_CONFIG_PATH"
    @echo "ALCHEMIST_DB_PATH        SQLite database path"
    @echo "                         Linux/macOS default: ~/.config/alchemist/alchemist.db"
    @echo "                         Windows default:     %APPDATA%\\Alchemist\\alchemist.db"
    @echo "ALCHEMIST_DATA_DIR       Override data directory for the DB file"
    @echo "ALCHEMIST_CONFIG_MUTABLE Allow runtime config writes (default: true)"
    @echo "XDG_CONFIG_HOME          Respected on Linux/macOS if set"
    @echo "RUST_LOG                 Log level (e.g. info, debug, alchemist=trace)"
