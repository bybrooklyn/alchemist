# Changelog

All notable changes to this project will be documented in this file.

## [v0.2.5] - 2026-01-11

### 🛠 Fixes
- **Dashboard Crash**: Fixed a critical bug where the dashboard would render as a blank screen if GPU utilization was `null`. Added strict null checks before `toFixed()` calls in `ResourceMonitor.tsx`.
- **Animation Glitch**: Resolved an issue where the "Engine Status" button would fly in from the top-left corner on page navigation. Implemented unique `layoutId` generation using `useId()` to maintain the morph animation while preventing cross-page artifacts.
- **Migration Checksum**: Fixed a startup error caused by a modified migration file. Reverted the original migration to restore checksum integrity and created a new migration for the version bump.

### ⚡️ Improvements
- **Resource Monitor Layout**: Repositioned the GPU Usage section to appear between "Active Jobs" and "Uptime" for better logical flow.
- **Animation Timing**: Adjusted staggered animation delays in the Resource Monitor to match the new layout order.

### 📚 Documentation
- **Codebase Overview**: Added `codebase_overview.md` explaining the monolith architecture (Rust + API + Frontend) and directory structure.
- **Migration Policy**: Updated `MIGRATIONS.md` to explicitly forbid modifying existing migration files to prevent checksum errors.
- **Walkthrough**: Updated `walkthrough.md` with detailed debugging logs and verification steps for all recent changes.

### 🏗 Infrastructure
- **Version Bump**: Updated project version to `0.2.5` in `Cargo.toml`, `web/package.json`, and `VERSION`.
- **Database**: Established `0.2.5` as the new minimum compatible version schema baseline.
