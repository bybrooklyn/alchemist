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

# Build frontend assets, then start the backend server
dev: web-build
    @just run

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
    cd web && bun install --frozen-lockfile && bun run typecheck && echo "── Frontend build ──" && bun run build
    @echo "All checks passed ✓"

# Rust checks only (faster)
check-rust:
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo check --all-targets

# Frontend checks only
check-web:
    cd web && bun install --frozen-lockfile && bun run typecheck && bun run build
    cd web-e2e && bun install --frozen-lockfile && bun run test

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
        rm -f "$DB" "$DB-wal" "$DB-shm"; \
        echo "Done — next run will re-apply migrations."

# Wipe dev database AND config (full clean slate, triggers setup wizard)
db-reset-all:
    @DB="${ALCHEMIST_DB_PATH:-${XDG_CONFIG_HOME:-$HOME/.config}/alchemist/alchemist.db}"; \
        CFG="${ALCHEMIST_CONFIG_PATH:-${XDG_CONFIG_HOME:-$HOME/.config}/alchemist/config.toml}"; \
        echo "Deleting $DB and $CFG"; \
        rm -f "$DB" "$DB-wal" "$DB-shm" "$CFG"; \
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

# Build multi-arch image (requires buildx; add --push to push to registry)
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

# Bump version across repo version files only (e.g. just bump 0.3.0 or just bump 0.3.0-rc.1)
bump NEW_VERSION:
    @echo "Bumping to {{NEW_VERSION}}..."
    bash scripts/bump_version.sh {{NEW_VERSION}}

# Checkpoint dirty local work with confirmation, then bump, validate, commit, tag, and push
# (blocks behind/diverged remote state; e.g. just update 0.3.0-rc.1 or just update v0.3.0-rc.1)
update NEW_VERSION:
    @RAW_VERSION="{{NEW_VERSION}}"; \
    NEW_VERSION="${RAW_VERSION#v}"; \
    CURRENT_VERSION="{{VERSION}}"; \
    TAG="v${NEW_VERSION}"; \
    BRANCH="$(git branch --show-current)"; \
    if [ -z "${NEW_VERSION}" ]; then \
        echo "error: version must not be empty" >&2; \
        exit 1; \
    fi; \
    if [ "${NEW_VERSION}" = "${CURRENT_VERSION}" ]; then \
        echo "error: version ${NEW_VERSION} is already current" >&2; \
        exit 1; \
    fi; \
    if [ -z "${BRANCH}" ]; then \
        echo "error: detached HEAD is not supported for just update" >&2; \
        exit 1; \
    fi; \
    if ! git remote get-url origin >/dev/null 2>&1; then \
        echo "error: origin remote does not exist" >&2; \
        exit 1; \
    fi; \
    if [ -n "$(git status --porcelain)" ]; then \
        echo "── Local changes detected ──"; \
        git status --short; \
        if [ ! -r /dev/tty ]; then \
            echo "error: interactive confirmation requires a TTY" >&2; \
            exit 1; \
        fi; \
        printf "Checkpoint current local changes before release? [y/N] " > /dev/tty; \
        read -r RESPONSE < /dev/tty; \
        case "${RESPONSE}" in \
            [Yy]|[Yy][Ee][Ss]) \
                git add -A; \
                git commit -m "chore: checkpoint before release ${TAG}"; \
                ;; \
            *) \
                echo "error: aborted because local changes were not checkpointed" >&2; \
                exit 1; \
                ;; \
        esac; \
    fi; \
    git fetch --quiet --prune --tags origin; \
    if git show-ref --verify --quiet "refs/remotes/origin/${BRANCH}"; then \
        LOCAL_HEAD="$(git rev-parse HEAD)"; \
        REMOTE_HEAD="$(git rev-parse "refs/remotes/origin/${BRANCH}")"; \
        BASE_HEAD="$(git merge-base HEAD "refs/remotes/origin/${BRANCH}")"; \
        if [ "${LOCAL_HEAD}" != "${REMOTE_HEAD}" ]; then \
            if [ "${BASE_HEAD}" = "${LOCAL_HEAD}" ]; then \
                echo "error: branch ${BRANCH} is behind origin/${BRANCH}; pull before running just update" >&2; \
                exit 1; \
            fi; \
            if [ "${BASE_HEAD}" != "${REMOTE_HEAD}" ]; then \
                echo "error: branch ${BRANCH} has diverged from origin/${BRANCH}; reconcile it before running just update" >&2; \
                exit 1; \
            fi; \
        fi; \
    fi; \
    if git rev-parse -q --verify "refs/tags/${TAG}" >/dev/null 2>&1; then \
        echo "error: local tag ${TAG} already exists" >&2; \
        exit 1; \
    fi; \
    if git ls-remote --exit-code --tags origin "refs/tags/${TAG}" >/dev/null 2>&1; then \
        echo "error: remote tag ${TAG} already exists on origin" >&2; \
        exit 1; \
    fi; \
    echo "── Bump version to ${NEW_VERSION} ──"; \
    bash scripts/bump_version.sh "${NEW_VERSION}"; \
    echo "── Rust format ──"; \
    cargo fmt --all -- --check; \
    echo "── Rust clippy ──"; \
    cargo clippy --locked --all-targets --all-features -- -D warnings; \
    echo "── Rust check ──"; \
    cargo check --locked --all-targets --all-features; \
    echo "── Rust tests ──"; \
    cargo test --locked --all-targets -- --test-threads=4; \
    echo "── Actionlint ──"; \
    actionlint .github/workflows/*.yml; \
    echo "── Web verify ──"; \
    (cd web && bun install --frozen-lockfile && bun run typecheck && bun run build); \
    echo "── E2E reliability ──"; \
    E2E_PORT=""; \
    for port in $(seq 4173 4273); do \
        if python3 -c "import socket, sys; s = socket.socket(); sys.exit(0 if s.connect_ex(('127.0.0.1', $port)) != 0 else 1)" >/dev/null 2>&1; then \
            E2E_PORT="$port"; \
            break; \
        fi; \
    done; \
    if [ -z "$E2E_PORT" ]; then \
        echo "error: no free web-e2e port found in 4173-4273" >&2; \
        exit 1; \
    fi; \
    echo "Using web-e2e port ${E2E_PORT}"; \
    (cd web-e2e && bun install --frozen-lockfile && ALCHEMIST_E2E_PORT="${E2E_PORT}" bun run test:reliability); \
    PACKAGE_FILES=(); \
    while IFS= read -r line; do [ -n "$line" ] && PACKAGE_FILES+=("$line"); done < <(git ls-files -- 'package.json' '*/package.json'); \
    CHANGED_TRACKED=(); \
    while IFS= read -r line; do [ -n "$line" ] && CHANGED_TRACKED+=("$line"); done < <(git diff --name-only --); \
    if [ "${#CHANGED_TRACKED[@]}" -eq 0 ]; then \
        echo "error: bump completed but no tracked files changed" >&2; \
        exit 1; \
    fi; \
    for file in "${CHANGED_TRACKED[@]}"; do \
        case "${file}" in \
            VERSION|Cargo.toml|Cargo.lock|package.json|*/package.json) ;; \
            *) \
                echo "error: unexpected tracked change after validation: ${file}" >&2; \
                exit 1; \
                ;; \
        esac; \
    done; \
    git add -- VERSION Cargo.toml Cargo.lock; \
    if [ "${#PACKAGE_FILES[@]}" -gt 0 ]; then \
        git add -- "${PACKAGE_FILES[@]}"; \
    fi; \
    if git diff --cached --quiet; then \
        echo "error: no version files were staged for commit" >&2; \
        exit 1; \
    fi; \
    echo "── Commit release ──"; \
    git commit -m "chore: release ${TAG}"; \
    echo "── Tag release ──"; \
    git tag -a "${TAG}" -m "${TAG}"; \
    echo "── Push branch ──"; \
    git push origin "${BRANCH}"; \
    echo "── Push tag ──"; \
    git push origin "refs/tags/${TAG}"

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
# UTILITIES
# ─────────────────────────────────────────

# Format all Rust code
fmt:
    cargo fmt --all

# Clean all build artifacts
clean:
    cargo clean
    rm -rf web/dist web/node_modules web-e2e/node_modules

# Count lines of source code
loc:
    @echo "── Rust ──"
    @count=0; \
        if [ -d src ]; then \
            count=$(find src -type f -name "*.rs" -exec cat {} + | wc -l | tr -d '[:space:]'); \
        fi; \
        printf "%8s total\n" "$count"
    @echo "── Frontend ──"
    @count=0; \
        if [ -d web/src ]; then \
            count=$(find web/src -type f \( -name "*.ts" -o -name "*.tsx" -o -name "*.astro" \) -exec cat {} + | wc -l | tr -d '[:space:]'); \
        fi; \
        printf "%8s total\n" "$count"
    @echo "── Tests ──"
    @count=0; \
        paths=(); \
        [ -d tests ] && paths+=(tests); \
        [ -d web-e2e/tests ] && paths+=(web-e2e/tests); \
        if [ ${#paths[@]} -gt 0 ]; then \
            count=$(find "${paths[@]}" -type f \( -name "*.rs" -o -name "*.ts" \) -exec cat {} + | wc -l | tr -d '[:space:]'); \
        fi; \
        printf "%8s total\n" "$count"

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
