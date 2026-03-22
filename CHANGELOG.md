# Changelog

All notable changes to this project will be documented in this file.

## [v0.2.10-rc.5] - 2026-03-22

### Runtime & Queue Control
- Engine runtime modes now support `background`, `balanced`, and `throughput`, with manual concurrency overrides and drain/resume controls exposed through the API and dashboard header.
- Engine status reporting now includes pause source, drain state, concurrent-limit metadata, and mode visibility for troubleshooting.
- Queue management continued to harden with safer active-job controls, clearer failure surfacing, and better per-job operational feedback.

### Media Processing & Library Health
- VAAPI-first Intel handling, remux planning, subtitle sidecars, and library health issue reporting were expanded across the planner, FFmpeg integration, and dashboard.
- Hardware detection and probe logging were improved to make CPU/GPU backend selection and diagnostics easier to understand.
- Stream rules landed for commentary stripping, audio-language filtering, and keeping only the default audio track when needed.

### Paths, Setup & Docs
- Default config and database paths were normalized around the `alchemist` runtime home, and the repo now ships a `justfile` for common dev, release, Docker, and database workflows.
- The docs site moved onto Starlight and now builds locally with a proper content config, localized collection wiring, and a corrected splash-page schema.
- Release automation now bumps repo-wide version manifests, supports checkpoint commits before release validation, and isolates web-e2e onto a separate port so a local server on `3000` does not block release verification.

### CI/CD & Release Tooling
- Docker publishing is now gated behind successful validation, and local release verification covers Rust checks/tests, frontend verification, docs build, `actionlint`, and the reliability Playwright suite.
- `just update` does not create the release commit, git tag, or push until the full validation gate passes.

## [v0.2.10-rc.2] - 2026-03-21

### Stability & Reliability
- VMAF quality gating: encodes falling below a configurable minimum score are now rejected rather than silently promoted.
- Exponential retry backoff for failed jobs: 5 / 15 / 60 / 360 minute delays based on attempt count prevent tight failure loops.
- Orphaned temp file cleanup on startup: interrupted encodes no longer leave `.alchemist.tmp` files on disk indefinitely.
- Log table pruning: configurable retention period (default 30 days) prevents unbounded log growth on busy servers.
- Auth session cleanup: expired sessions are pruned on startup and every 24 hours.
- Resource endpoint caching: `/api/system/resources` is cached for 500ms to prevent redundant OS probes from multiple open browser tabs.

### New Features
- Per-library profiles: each watch folder can have its own transcoding profile, with four built-in presets (Space Saver, Quality First, Balanced, Streaming) usable as starting points.
- Storage savings dashboard: the Stats page now shows total space recovered, average reduction percentage, a savings-over-time chart, and per-codec breakdowns.
- Library Doctor: scan your library for corrupt or broken files directly from System Settings.
- `/api/jobs` added as a canonical alias for `/api/jobs/table`.

### Beginner Experience
- Plain-English skip reasons: skipped jobs now show a human-readable explanation with technical detail available in an expandable section.
- Auto-redirect to the setup wizard for first-time users with no watch directories configured.
- Setup wizard field descriptions: CRF, BPP, concurrent jobs, and CPU preset now include plain-English explanations inline.
- Telemetry is now opt-in by default, with a detailed explanation of exactly what is collected.

### Job Management
- Skipped tab: dedicated tab in the Job Manager for skipped jobs.
- Archived tab: cleared completed jobs are now visible in an Archived tab rather than disappearing permanently.
- Sort controls: the job list can now be sorted by last updated, date added, file name, or file size.

### UI & Design
- Font updated from Space Grotesk to DM Sans.
- Sidebar active state redesigned: a left accent bar replaces the filled background.
- Border radius tightened throughout with a more consistent scale across cards, buttons, and badges.
- Setup wizard refactored into composable step components.

### Infrastructure
- CI/CD workflows fully rewritten: Rust caching, TypeScript typechecking, and a frontend build shared across all platforms.
- Multi-arch Docker images are now published for `linux/amd64` and `linux/arm64`.
- Release binaries now ship as `.tar.gz` archives with SHA256 checksums; AppImage and `.app` bundles were removed.
- Dockerfile now uses the stable Rust image with pinned FFmpeg checksums.
- E2E test coverage added for all new features.

## [v0.2.10-rc.1] - 2026-03-07
- Job lifecycle safety hardening: queued vs active cancel handling, active-job delete/restart blocking, batch-action conflict reporting, and stricter status/stat persistence.
- Output handling now supports mirrored `output_root` destinations plus temp-file promotion so replace mode preserves the last good artifact until encode, size, and quality gates pass.
- Scheduler, setup, and watch-folder parity updates shipped together: immediate schedule reevaluation, Intel Arc H.264 detection fix, H.264 setup option, canonicalized watch folders, and recursive watch configuration in the UI.
- Jobs and settings UX now expose per-job priority controls, output-root file settings, active-job-safe actions, and the Astro router deprecation cleanup.
- CI/CD rewrite for `0.2.10-rc.1`: cached Rust checks, frontend typecheck/build validation, multi-arch Docker publishing, and unified prerelease metadata handling across workflows.
- Release packaging cleanup: Linux and macOS now ship plain `.tar.gz` binaries, Windows ships `.exe`, and every release asset includes a SHA256 checksum file.

## [v0.2.9] - 2026-03-06
- Runtime reliability pass: watcher/scanner hardening, resilient event consumers, config reload improvements, and live hardware refresh.
- Admin UX refresh across dashboard, settings, setup, logs, jobs, charts, and system status with stronger error handling and feedback.
- Frontend workflow standardized on Bun, Playwright reliability coverage added under `web-e2e`, and deploy/docs/container updates shipped together.

## [v0.2.8] - 2026-01-12
- Setup wizard auth fixes, scheduler time validation, and watcher reliability improvements.
- DB stability pass (WAL, FK enforcement, indexes, session cleanup, legacy watch_dirs compatibility).
- Build pipeline updates (rustls for reqwest, cross-platform build script, WiX workflow fix).
- Documentation and design philosophy updates.
- More themes!!

## [v0.2.5] - 2026-01-11

###  Fixes
- **Dashboard Crash**: Fixed a critical bug where the dashboard would render as a blank screen if GPU utilization was `null`. Added strict null checks before `toFixed()` calls in `ResourceMonitor.tsx`.
- **Animation Glitch**: Resolved an issue where the "Engine Status" button would fly in from the top-left corner on page navigation. Implemented unique `layoutId` generation using `useId()` to maintain the morph animation while preventing cross-page artifacts.
- **Migration Checksum**: Fixed a startup error caused by a modified migration file. Reverted the original migration to restore checksum integrity and created a new migration for the version bump.

###  Improvements
- **Resource Monitor Layout**: Repositioned the GPU Usage section to appear between "Active Jobs" and "Uptime" for better logical flow.
- **Animation Timing**: Adjusted staggered animation delays in the Resource Monitor to match the new layout order.

###  Documentation
- **Codebase Overview**: Added `codebase_overview.md` explaining the monolith architecture (Rust + API + Frontend) and directory structure.
- **Migration Policy**: Updated `MIGRATIONS.md` to explicitly forbid modifying existing migration files to prevent checksum errors.
- **Walkthrough**: Updated `walkthrough.md` with detailed debugging logs and verification steps for all recent changes.

###  Infrastructure
- **Version Bump**: Updated project version to `0.2.5` in `Cargo.toml`, `web/package.json`, and `VERSION`.
- **Database**: Established `0.2.5` as the new minimum compatible version schema baseline.
