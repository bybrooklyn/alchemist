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

### Base URL / Subpath Support
- `ALCHEMIST_BASE_URL` and matching config support
- Router nesting under a configured path prefix
- Frontend fetches, redirects, navigation, and SSE path generation updated for subpaths

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

---

## Active Priorities

### Engine Lifecycle Controls
- Finish and harden restart/shutdown semantics from the About/header surface
- Restart must reset the engine loop without re-execing the process
- Shutdown must cancel active jobs and exit cleanly
- Add final backend and Playwright coverage for lifecycle transitions

### Planner and Lifecycle Documentation
- Document planner heuristics and stable skip/transcode/remux decision boundaries
- Document hardware fallback rules and backend selection semantics
- Document pause, drain, restart, cancel, and shutdown semantics from actual behavior

### Per-File Encode History
- Show full attempt history in job detail, grouped by canonical file identity
- Include outcome, encode stats, and failure reason where available
- Make retries, reruns, and settings-driven requeues legible

### Behavior-Preserving Refactor Pass
- Decompose `web/src/components/JobManager.tsx` without changing current behavior
- Extract shared formatting logic
- Clarify SSE vs polling ownership
- Add regression coverage before deeper structural cleanup

### AMD AV1 Validation
- Validate Linux VAAPI and Windows AMF AV1 paths on real hardware
- Confirm encoder selection, fallback behavior, and defaults
- Keep support claims conservative until validation is real

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
