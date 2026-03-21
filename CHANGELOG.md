# Changelog

All notable changes to this project will be documented in this file.

## [v0.2.10-rc.1] - 2026-03-07
- Job lifecycle safety hardening: queued vs active cancel handling, active-job delete/restart blocking, batch-action conflict reporting, and stricter status/stat persistence.
- Output handling now supports mirrored `output_root` destinations plus temp-file promotion so replace mode preserves the last good artifact until encode, size, and quality gates pass.
- Scheduler, setup, and watch-folder parity updates shipped together: immediate schedule reevaluation, Intel Arc H.264 detection fix, H.264 setup option, canonicalized watch folders, and recursive watch configuration in the UI.
- Jobs and settings UX now expose per-job priority controls, output-root file settings, active-job-safe actions, and the Astro router deprecation cleanup.
- RC release hardening for `0.2.10-rc.1`: version metadata synced across app, Docker, and web artifacts; prerelease workflow behavior added; release instructions updated around main-driven RC Docker tags and tag-driven prerelease assets.

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
