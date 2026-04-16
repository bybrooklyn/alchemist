# Alchemist Backlog

Current and future work for Alchemist, organized around the
actual repo state rather than historical priorities.

Alchemist should remain an automation-first media
optimization tool, not drift into a general-purpose media
workbench.

---

## Implemented / In Progress

These items now exist in the repo and should be treated as
current product surface that still needs hardening,
documentation, or iteration.

### Conversion / Remux Workflow
- Dedicated **Convert** page for single-file upload-driven conversion
- Probe-driven UI with container, video, audio, subtitle, and remux-only controls
- FFmpeg command preview
- Temporary upload/output lifecycle under `~/.config/alchemist/temp`
- Reuse of the existing queue and worker system
- Status polling and download flow
- Treat this as an experimental utility, not a second core
  product track

### Notification Platform Expansion
- Provider-specific notification target model backed by `config_json`
- Discord webhook, Discord bot, Gotify, generic webhook, Telegram, and email targets
- Richer event taxonomy:
  - `encode.queued`
  - `encode.started`
  - `encode.completed`
  - `encode.failed`
  - `scan.completed`
  - `engine.idle`
  - `daily.summary`
- Per-target event filtering
- Daily summary scheduling via `daily_summary_time_local`

### API Token Authentication + API Docs
- Named static API tokens with `read_only` and `full_access` classes
- Hash-only token storage, plaintext shown once at creation
- Token management endpoints and Settings UI
- Hand-maintained OpenAPI contract plus human API docs

### Distribution Foundation
- In-repo distribution metadata sources for:
  - Homebrew
  - AUR
  - Windows update-check metadata
- Release workflow renders package metadata from release assets/checksums
- Windows in-app update check against GitHub Releases

### Expanded Library Intelligence
- Duplicate groups remain
- Storage-focused recommendation categories added:
  - remux-only opportunities
  - wasteful audio layouts
  - commentary/descriptive-track cleanup candidates
- Direct actions now exist for queueing remux recommendations and opening duplicate candidates in the shared job-detail flow

### Engine Lifecycle + Planner Docs
- Runtime drain/restart controls exist in the product surface
- Backend and Playwright lifecycle coverage now exists for the current behavior
- Planner and engine lifecycle docs are in-repo and should now be kept in sync with shipped semantics rather than treated as missing work

### Jobs UI Refactor / In Flight
- `JobManager` has been decomposed into focused jobs subcomponents and controller hooks
- SSE ownership is now centered in a dedicated hook and job-detail controller flow
- Treat the current jobs UI surface as shipping product that still needs stabilization and regression coverage, not as a future refactor candidate

---

## Active Priorities

### `0.3.1` RC Stability Follow-Through
- Keep the current in-flight backend/frontend/test delta focused on reliability, upgrade safety, and release hardening
- Expand regression coverage for resume/restart/cancel flows, job-detail refresh semantics, settings projection, and intelligence actions
- Keep release docs, changelog entries, and support wording aligned with what the RC actually ships

### Per-File Encode History Follow-Through
- Attempt history now exists in job detail, but it is still job-scoped rather than grouped by canonical file identity
- Next hardening pass should make retries, reruns, and settings-driven requeues legible across a file’s full history
- Include outcome, encode stats, and failure reason where available without regressing the existing job-detail flow

### AMD AV1 Validation
- Validate Linux VAAPI and Windows AMF AV1 paths on real hardware
- Confirm encoder selection, fallback behavior, and defaults
- Keep support claims conservative until validation is real
- Deferred from the current `0.3.1-rc.5` automated-stability pass; do not broaden support claims before this work is complete

---

## Later

### Documentation
- Architecture diagrams
- Contributor walkthrough improvements
- Video tutorials for common workflows

### Code Quality
- Increase coverage for edge cases
- Add property-based tests for codec parameter generation
- Add fuzzing for FFprobe parsing

### Planning / Simulation Mode
- Promote this only after the current Active Priorities are done
- Single-config dry run first
- No comparison matrix or scenario planner until the first simple flow proves useful

### Audio Normalization
- Add opt-in EBU R128 loudness normalization during transcode
- Surface loudness metrics in job detail
- Keep copy-mode bypass behavior explicit
- Keep this secondary unless it clearly supports the automation-first mission

### Auto-Priority Rules
- Add explainable enqueue-time priority automation
- Manual priority overrides must still win
- Matched rules must be visible in the UI to keep queue behavior trustworthy

### UI Improvements
- Tighten settings and detail-panel consistency
- Improve dense forms, empty states, and action affordances
- Keep this narrowly scoped to automation-supporting UX problems

### Keyboard Shortcuts
- Add a concrete shortcut set for common jobs/logs/conversion actions
- Avoid a vague “shortcut layer everywhere” rollout
- First likely cut if scope pressure appears

### Features from DESIGN_PHILOSOPHY.md
- Add batch job templates

### Distribution Follow-Ons
- Flatpak / Snap packaging
- Additional installer polish beyond the current Windows update-check flow
- Only promote these if they become strategically important

---

## Out of Scope

- Custom FFmpeg flags / raw flag injection
- Distributed encoding across multiple machines
- Features that turn Alchemist into a general-purpose media
  workbench
- Fuzzy media-management intelligence that drifts away from storage quality and encode operations
