# Alchemist Backlog

Future improvements and features to consider for the project.

## High Priority

### Planning / Simulation Mode
- Add a first-class simulation flow that answers what Alchemist would transcode, remux, or skip without mutating the library
- Show estimated total bytes recoverable, action counts, top skip reasons, and per-file predicted actions
- Support comparing current settings against alternative profiles, codec targets, or threshold snapshots
- Reuse the scanner, analyzer, and planner, but stop before executor and promotion stages

### E2E Test Coverage
- Expand Playwright tests for more UI flows
- Test job queue management scenarios
- Test error states and recovery flows

### AMD AV1 Validation
- Validate and tune the existing AMD AV1 paths on real hardware
- Cover Linux VAAPI and Windows AMF separately
- Verify encoder selection, fallback behavior, and quality/performance defaults
- Do not treat this as support-from-scratch: encoder wiring and hardware detection already exist

## Medium Priority

### Decision Clarity
- Replace loose skip/failure reason strings with structured UI/API payloads that include a code, plain-English summary, measured values, and operator guidance
- Show concise skip/failure summaries before raw logs in the job detail panel
- Make the jobs list communicate skip/failure class at a glance

### Library Intelligence
- Expand recommendations beyond duplicate detection into remux-only opportunities, wasteful audio layouts, commentary/descriptive-track cleanup, and duplicate-ish title variants
- Keep the feature focused on storage and library quality, not general media management

### Performance Optimizations
- Profile scanner/analyzer hot paths before changing behavior
- Only tune connection pooling after measuring database contention under load
- Consider caching repeated FFprobe calls on identical files if profiling shows probe churn is material

### Audio Normalization
- Apply EBU R128 loudness normalization to audio streams during transcode
- Target: -23 LUFS integrated, -1 dBTP true peak (broadcast standard)
- Opt-in per library profile, disabled by default
- Implemented via `loudnorm` FFmpeg filter — no new dependencies
- Two-pass mode for accurate results; single-pass for speed
- Should surface loudness stats (measured LUFS, correction applied) in
  the job detail panel alongside existing encode stats
- Do not normalize if audio is being copied (copy mode bypasses this)

### Retry Backoff Visibility
- The retry backoff schedule already exists in the backend
  (5 min / 15 min / 60 min / 360 min by attempt count) but is
  completely invisible in the UI
- Failed jobs waiting to retry should show a clear countdown or
  timestamp: "Retrying in 12 minutes" or "Next retry at 3:45 PM"
- The Jobs page Failed tab should distinguish between
  "failed — will retry" and "failed — gave up" at a glance
- The job detail panel should show attempt count and next retry time

### UI Improvements
- Improve mobile responsiveness
- Add keyboard shortcuts for common actions

### Notification Improvements
- **Granular event types** — current events are too coarse. Add:
    - `encode.started` — job moved from queued to encoding
    - `encode.completed` — with savings summary (size before/after)
    - `encode.failed` — with failure reason included in payload
    - `scan.completed` — N files discovered, M queued
    - `engine.idle` — queue drained, nothing left to process
    - `daily.summary` — opt-in digest of the day's activity
- **Per-target event filtering** — each notification target should
  independently choose which events it receives. Currently all targets
  get the same events. A Discord webhook might want everything; a
  phone webhook might only want failures.
- **Richer payloads** — completed job notifications should include
  filename, input size, output size, space saved, and encode time.
  Currently the payload is minimal.
- **Add Telegram integration** — bot token + chat ID, same event
  model as Discord. No new dependencies needed (reqwest already present).
- **Add email support** — SMTP with TLS. Lower priority than Telegram.
  Most self-hosters already have Discord or Telegram.

## Low Priority

### Features from DESIGN_PHILOSOPHY.md
- Consider WebSocket alternative to SSE for bidirectional communication
- Add batch job templates

### Code Quality
- Increase test coverage for edge cases
- Add property-based testing for codec parameter generation
- Add fuzzing for FFprobe output parsing

### Documentation
- Add architecture diagrams
- Add contributor guide with development setup
- Video tutorials for common workflows
- API client examples in multiple languages

### Distribution
- Add Homebrew formula
- Add AUR package
- Add Flatpak/Snap packages
- Improve Windows installer (WiX) with auto-updates

## Completed (Recent)

- [x] Split server.rs into modules
- [x] Add API versioning (/api/v1/)
- [x] Add typed broadcast channels
- [x] Add security headers middleware
- [x] Add database query timeouts
- [x] Add config file permission check
- [x] Handle SSE lagged events in frontend
- [x] Create FFmpeg integration tests
- [x] Expand documentation site
- [x] Create OpenAPI spec
- [x] Pin MSRV in Cargo.toml
- [x] Add schema versioning for migrations
- [x] Enable SQLite WAL mode
- [x] Add theme persistence and selection
- [x] Add job history filtering and search
- [x] Add subtitle extraction sidecars
