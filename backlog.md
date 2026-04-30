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
- Discord webhook, Discord bot, Gotify, ntfy, generic webhook, Telegram, and email targets
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
- Shift-click range select now works across the jobs table
- Job detail now groups encode attempts into distinct per-file runs so reruns and retries stay legible
- Treat the current jobs UI surface as shipping product that still needs stabilization and regression coverage, not as a future refactor candidate

### Operational Self-Hosting
- Runtime settings now expose one-click SQLite backup as a gzip download backed by SQLite's online snapshot path
- README / release documentation should treat database backup as shipped product surface, not roadmap work

---

## Active Priorities

### `0.3.1` RC Stability Follow-Through
- Keep the current in-flight backend/frontend/test delta focused on reliability, upgrade safety, and release hardening
- Expand regression coverage for resume/restart/cancel flows, job-detail refresh semantics, settings projection, and intelligence actions
- Keep release docs, changelog entries, and support wording aligned with what the RC actually ships

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

### Audio Normalization (ENC-1)
- Add opt-in EBU R128 loudness normalization during transcode
- Surface loudness metrics in job detail
- Keep copy-mode bypass behavior explicit
- Keep this secondary unless it clearly supports the automation-first mission

### Auto-Priority Rules
- Add explainable enqueue-time priority automation
- Manual priority overrides must still win
- Matched rules must be visible in the UI to keep queue behavior trustworthy
- See also AUTO-1 (broader conditional-profile rules engine); Auto-Priority is a subset of that design space

### UI Improvements
- Tighten settings and detail-panel consistency
- Improve dense forms, empty states, and action affordances
- Rework Appearance/theme selection later: simplify the page structure, fix light-theme token contrast, add a reliable text-on-accent token, and verify cards/buttons across light profiles
- Keep this narrowly scoped to automation-supporting UX problems

### Keyboard Shortcuts (UX-2)
- Add a concrete shortcut set for common jobs/logs/conversion actions
- Avoid a vague "shortcut layer everywhere" rollout
- First likely cut if scope pressure appears

### Features from DESIGN_PHILOSOPHY.md
- Add batch job templates

### Distribution Follow-Ons
- Flatpak / Snap packaging
- Additional installer polish beyond the current Windows update-check flow
- Only promote these if they become strategically important

### *arr Webhook Ingress (INT-1)
- Accept Sonarr/Radarr Download and Upgrade webhook payloads at `POST /api/webhooks/arr`
- Translate container-relative paths through a configurable prefix; enqueue an analyze job on receipt
- Authenticate via a dedicated `arr_webhook` API token class
- Highest-intent integration for the dominant self-hosted media stack; promote when scheduling allows

### Jellyfin / Plex Refresh Hook (INT-2)
- Fire a library-refresh API call after finalization so downstream players pick up the new file immediately
- Configure per target (server URL, API key, section ID); bundle multiple finalizations within a short window
- Start Jellyfin-only behind a config flag, add Plex after the pattern is proven

### Prometheus Metrics (OBS-1)
- Expose a `GET /metrics` endpoint behind a config flag (`metrics.enabled`, `metrics.bind`)
- Export queue depth by state, encodes completed by codec, bytes saved, encode duration histogram, encoder selection counts, pipeline error counts
- Gives self-hosters first-class Grafana dashboards without a custom scraper

### Structured JSON Logging (OBS-2)
- Add `log_format = "text" | "json"` config + `ALCHEMIST_LOG_FORMAT` env var
- JSON mode emits `tracing-subscriber::fmt::json()` output for Loki / Elastic / Datadog ingestion
- Small scope; trivial implementation on top of the existing subscriber

### FFprobe Result Cache (PERF-1)
- Add a `media_probe_cache` table keyed by `(path, mtime, size)` storing the serialized analyzer result
- On scan, reuse cache hits and re-probe only on mtime/size mismatch; invalidate across FFmpeg versions
- Large-library rescans are probe-dominated — expected wall-time win on slow disks

### Undo Last Encode (F-1)
- Capture the original file's backup path on replace-mode encodes; keep for a configurable retention window
- Expose "Restore original" in job detail; swap atomically, transition the job to a new terminal `Reverted` state
- Flagship trust feature; aligns with "never overwrite by default" design principle

### Library Plan Preview (F-2)
- `POST /api/library/preview` runs the planner in dry-run over a path and returns skip/remux/encode counts plus estimated savings
- Surface as a "Preview" action in Watch Folders and the setup wizard; nothing is enqueued
- Cap scope (first N files or fixed timeout) and show partial-preview state clearly

### Pipeline Self-Test (OP-2)
- Ship a tiny embedded public-domain sample clip and a `alchemist selftest` CLI + UI button
- Runs the full pipeline with the user's profile against the sample; reports stage, encoder, wall time, and success
- Lets users tell "it's my config" apart from "the pipeline is broken" without guesswork

### Chapter Preservation Check (ENC-2)
- Record source chapter count in the analyzer; verify output chapter count during finalization
- Attach a non-fatal warning to jobs that silently drop chapters and surface it in the detail modal
- Catches a class of silent-quality-loss bugs that users otherwise notice weeks later

---

## Out of Scope

- Custom FFmpeg flags / raw flag injection
- Distributed encoding across multiple machines
- Features that turn Alchemist into a general-purpose media
  workbench
- Fuzzy media-management intelligence that drifts away from storage quality and encode operations
