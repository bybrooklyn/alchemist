# Changelog

All notable changes to this project will be documented in this file.

## [0.3.0] - 2026-04-05

### Security
- Fixed a critical bug where a config parse failure on a configured instance would re-enable unauthenticated setup endpoints (filesystem browse, settings bundle) for any network client.
- Session cookies are no longer marked `Secure` by default, which was breaking login over plain HTTP/LAN. Opt in with `ALCHEMIST_COOKIE_SECURE=true` for reverse-proxy deployments.
- `/api/fs/*` filesystem browsing is now restricted to loopback connections only during the initial setup flow.
- Proxy header handling hardened with explicit trust configuration for reverse-proxy deployments.

### New Features

#### Library & Encoding
- **Per-library profiles** — each watch folder gets its own transcoding profile. Four built-in presets (Space Saver, Quality First, Balanced, Streaming) are ready to use or customize.
- **Container remuxing** — files already in the target codec but wrapped in MP4/MOV are remuxed to MKV losslessly, skipping a full re-encode.
- **Subtitle sidecar extraction** — text-based subtitle tracks (SRT, ASS, VTT) can be extracted as separate files alongside the output rather than muxed in.
- **Stream rules** — strip audio tracks by title keyword (e.g. commentary tracks), filter by language code, or keep only the default audio track.
- **VMAF quality gating** — encodes scoring below a configurable threshold are rejected and the source is preserved.
- **Library Intelligence** — duplicate detection surfaces files with matching stems across the library.
- **Library Doctor** — health scanning detects corrupt or broken files directly from System Settings.
- **Mirrored output root** — write transcoded files to a separate directory tree that mirrors the source structure, rather than alongside the source.

#### Job Management
- **Skipped tab** — dedicated tab for skipped jobs with structured skip reasons.
- **Archived tab** — cleared completed jobs are preserved in an Archived tab rather than disappearing permanently.
- **Sort controls** — sort the job list by last updated, date added, file name, or file size.
- **Per-job priority** — promote individual jobs up the queue from the job detail panel.
- **Retry countdown** — failed jobs waiting to retry show "Retrying in 47m", updated live every 30 seconds.
- **Structured skip and failure explanations** — skip reasons and failure summaries are stored as structured payloads with a code, plain-English summary, measured values, and operator guidance; surfaced in the job detail panel before the raw FFmpeg log.

#### Engine Control
- **Engine runtime modes** — Background (1 job), Balanced (half CPU count, capped at 4), and Throughput (half CPU count, uncapped). Manual concurrency and thread overrides available in the Advanced panel.
- **Drain mode** — stop accepting new jobs while letting active encodes finish cleanly.
- **Boot auto-analysis** — ffprobe runs on all queued jobs at startup so skip/transcode decisions are pre-computed before the engine starts.

### UI Redesign
- Removed page `h1` headers; replaced the old header block with a thin engine control strip showing the status dot, Start/Pause/Stop, mode pills, About, and Logout in one row.
- Dashboard restructured around a compact stat row, savings summary card, and a larger Recent Activity panel.
- Log viewer groups entries by job into collapsible sections; system-level log lines render inline between groups.
- Setup wizard rebuilt inside the main app shell with a grayed sidebar, 2px solar progress line, and a welcome step (logo + tagline + Get Started) before the admin account form.
- Library selection redesigned around a flat recommendation list with Add buttons, selected-folder chips, and a Browse/manual path option; the old preview panel was removed.
- Statistics page uses recharts `AreaChart` for savings over time and `BarChart` for codec breakdown, replacing custom CSS bars.
- Hardware settings merged into the Transcoding tab. Notifications and Automation merged into one tab.
- Mobile layout: hamburger sidebar overlay, jobs table collapses date/priority columns below `md` breakpoint, stat cards use a 2×2 grid on small screens.
- Font updated from Space Grotesk to DM Sans; sidebar active state uses a left accent bar; border radius scale tightened throughout.
- Design system token compliance pass across all settings components: toggle switches, form labels, and text-on-color elements now use helios tokens exclusively.
- Analyzing job rows show an indeterminate shimmer instead of a static 0.0% label.
- Poll-based job state updates no longer overwrite terminal states that arrived via SSE.

### Reliability & Stability
- Exponential retry backoff for failed jobs: 5 / 15 / 60 / 360 minute delays by attempt count.
- Orphaned temp file cleanup on startup: interrupted encodes and subtitle sidecar temp files no longer accumulate on disk.
- Fixed infinite analysis loop: jobs with an existing decision row are excluded from analysis batches, preventing transcodable jobs from being re-analyzed on every pass.
- Boot analysis processes jobs in batches of 100 from offset 0, fixing a pagination bug where transcodable jobs shifted out of later pages after earlier jobs were decided.
- Engine no longer auto-pauses when the queue empties; it stays Running and picks up new files as the watcher delivers them.
- Analysis semaphore serializes all analysis passes; watcher-triggered passes are dropped (not queued) when a pass is already running.
- Job stall detection added to surface encodes that stop making progress.
- Ctrl+C / SIGTERM exits cleanly after graceful shutdown. Background tasks no longer prevent process exit.
- Log table pruning: configurable retention period (default 30 days) prevents unbounded log growth.
- Auth session cleanup: expired sessions pruned on startup and every 24 hours.
- Resource endpoint caching: `/api/system/resources` cached 500ms to prevent redundant OS probes from multiple open tabs.
- `Drop` guard added to `in_flight_jobs` counter so it decrements correctly even on panic.
- Completed job detail no longer re-runs ffprobe on the source file; `encode_stats` is the authoritative source for post-encode metadata.

### Hardware & Encoding
- **Apple VideoToolbox** — encode commands now include `-allow_sw 1` (software fallback) and `format=yuv420p` (required pixel format), fixing all VideoToolbox encodes on macOS. HEVC output tagged as `hvc1` for Apple device compatibility.
- **Intel Arc** — VAAPI-first detection with `i915`/`xe` driver; QSV retained as last-resort fallback only.
- **Audio planning** — lossless codecs (TrueHD, MLP, DTS-HD, FLAC, PCM) trigger transcoding; standard Atmos/EAC3 at any bitrate now copies through without re-encoding.
- **libopus fallback** — audio transcoding for MKV now checks for `libopus` availability at runtime and falls back to AAC when it is absent (common on macOS FFmpeg builds).
- FFmpeg encode failures write the full error (last 20 lines of stderr) to the job log; failure explanations in the UI include VideoToolbox-specific patterns (`vt_compression`, `mediaserverd`, `no capable devices`).

### Backend Architecture
- Upgraded from Rust 2021 to **Rust 2024 edition**, MSRV set to 1.85.
- `sqlx` upgraded to 0.8 with `runtime-tokio-rustls`; `rand` upgraded to 0.9.
- Removed `async-trait`; all traits use native `async fn`. `trait-variant` added for object-safe `Arc<dyn ExecutionObserver>`.
- `server.rs` split into focused submodules: `auth`, `jobs`, `scan`, `settings`, `stats`, `system`, `sse`, `middleware`, `wizard`.
- `ffprobe` execution moved to `tokio::process::Command` with a 120-second timeout.
- Typed broadcast channels separate high-volume events (progress, logs) from low-volume system events (config, status).
- Poisoned cancellation lock recovery added to the orchestrator; oversized FFmpeg stderr lines truncated before logging.
- Invalid notification event JSON and invalid schedule day JSON now log a warning rather than silently disabling the target or treating it as empty.
- Database connection pool capped; OOM protection added to analysis batch queries via `LIMIT`/`OFFSET` pagination.

### Database
- `decisions` table extended with `reason_code` and `reason_payload_json` for structured skip reason storage.
- `job_failure_explanations` table added for structured failure explanations, with `legacy_summary` fallback for pre-0.3 rows.
- Index on `decisions(reason_code)` and `job_failure_explanations(code)` for fast filtering.
- All databases from v0.2.5 onwards upgrade automatically; no manual migration required.

### CI/CD & Tooling
- Nightly workflow: runs on every push to `main` after checks pass, builds all platforms, publishes `ghcr.io/brooklynloveszelda/alchemist:nightly` with `{VERSION}-nightly+{short-sha}` versioning.
- Shared reusable `build.yml` workflow so nightly and release builds use identical pipelines.
- `actionlint` added to `just release-check`.
- E2E reliability suite (`just test-e2e`) runs in CI after the frontend check passes.
- Windows contributor workflow documented and validated: `just install-w`, `just dev`, `just check`.
- `just release-check` covers fmt, clippy (`-D warnings -D clippy::unwrap_used -D clippy::expect_used`), tests, actionlint, web verify, docs build, E2E, and backend build in sequence.
- Release binaries ship as `.tar.gz` (Linux/macOS) and `.exe` (Windows), each with a SHA256 checksum. Multi-arch Docker images published for `linux/amd64` and `linux/arm64`.

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
