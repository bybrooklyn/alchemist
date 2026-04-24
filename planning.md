# Planning

Last updated: 2026-04-24

## Current Status

- `MIG-3`: shipped (v1)
  - v1 JSON error schema helper landed for initial endpoint set
  - 2026-04-24: high-traffic auth, middleware, settings, system, jobs, and ARR webhook paths use structured API errors
- `INT-3`: shipped
  - ntfy target exists in backend + settings UI
- `AUTO-2`: shipped (v1)
  - v1 global quiet-hours suppression landed in backend dispatch
  - 2026-04-24: settings API + UI exposure completed with validation, normalization, and coverage
- `IMPR-1`: shipped (v1)
  - Astro content collection scaffolding added for quality help content
- `PERF-1`: shipped (v1)
  - additive `media_probe_cache` schema + analyzer read/write path exist in current tree
- `INT-1`: shipped
  - added `POST /api/webhooks/arr` ingestion endpoint
  - added dedicated API token access level/scope `arr_webhook`
  - added `system.arr_path_translations` mapping in webhook ingest path resolution
  - ingress reuses existing enqueue-by-submitted-path flow for dedupe/output safety
  - added auth/scope + parsing + translation + enqueue integration tests

## Execution Log (2026-04-24)

### Setup sidebar visibility fix (completed)

- During setup, sidebar now shows the full tool list (including **Intelligence** and **Convert**) in the same grayed/disabled style.
- Change made in `web/src/components/SetupSidebar.astro` by extending the static nav item list to include the missing entries.
- Regression coverage added in `web-e2e/tests/setup-recovery.spec.ts` to assert the full disabled setup navigation.
- Verification command run: `just check-web` (pass, 83 Playwright tests).

### Queue kickoff started

- Parallel kickoff started for:
  - `sync-planning-baseline`
  - `advance-mig3-error-schema`
  - `finish-auto2-quiet-hours`
- Implementation detail:
  - `MIG-3`: `src/server/settings.rs` now moving notification/settings failure paths to structured API errors.
  - `AUTO-2`: notifications settings payload/response include `quiet_hours_enabled`, `quiet_hours_start_local`, `quiet_hours_end_local`; completed follow-through added config validation (`quiet hours start/end must differ` when enabled), quiet-hours canonicalization defaults/trim behavior in config migration/save path, notifications API endpoint regression tests for get/put normalization + rejection, and UI save/load hardening in `NotificationSettings.tsx` (save state + post-save re-fetch to align with persisted server values).
  - `AUTO-2` advanced buffering/per-target policies are intentionally deferred.

### `advance-mig3-error-schema` completion notes

- Completed remaining high-traffic MIG-3 error-shape conversions in:
  - `src/server/auth.rs`
  - `src/server/middleware.rs`
  - `src/server/system.rs`
  - `src/server/settings.rs`
- Replaced plain `(StatusCode, "message")` / `(StatusCode, err.to_string())` error returns with `api_error_response(status, code, message)` and stable machine codes (auth, rate-limit, schedule, file-settings, API token, and preference/config validation/save/read surfaces).
- Preserved status codes and user-facing message text where possible; only response shape changed to the structured `{ error: { code, message } }` contract.

### `implement-int1-arr-webhook` completion notes

- Wired `src/server/webhooks.rs` into router/middleware:
  - Added module registration + route: `POST /api/webhooks/arr`
  - Added ARR-only API token authorization branch in auth middleware
- Token model work:
  - Added `arr_webhook` access level projection in API token read/create paths
  - Stored as additive scope (`access_scope='arr_webhook'`) while preserving legacy `read_only` / `full_access` storage semantics
- Path handling:
  - Webhook payload path resolution supports imported file arrays, direct file paths, and relative-path joins via series/movie roots
  - `system.arr_path_translations` longest-prefix mapping is applied before enqueue
- Queue integration:
  - Reused `enqueue_job_from_submitted_path` directly (no duplicate enqueue rules)
  - Webhook responses now return structured API errors for rejected/failed enqueue attempts
- Test coverage added in `src/server/tests.rs`:
  - ARR token scope auth behavior
  - payload-without-path structured error
  - translation + enqueue + dedupe behavior (same path webhook repeated)

### `implement-perf1-probe-cache` progress notes

- Added additive migration `migrations/20260424160000_media_probe_cache.sql`.
- Added DB helpers in `src/db/probe_cache.rs` for lookup/update and read-through writes.
- Analyzer integration uses `(input_path, mtime_ns, size_bytes, ffprobe version)` as the cache key and falls back to live ffprobe on lookup/decode/write failures.
- Added focused DB helper regression coverage for cache key behavior and overwrite semantics.
- Updated v0.2.5 upgrade coverage to assert schema version 12 plus `api_tokens.access_scope` and `media_probe_cache`.

### `implement-impr1-content-collections` progress notes

- Added `web/src/content.config.ts` with a typed `help` collection.
- Added first content entry at `web/src/content/help/quality-vmaf.md`.
- Added `/help/quality` Astro page that renders quality help from the content collection.
- Linked Quality settings VMAF copy to the new help page.

### Verification checkpoint

- `just check-rust` passed.
- `just check-web` passed.
- `just test` passed.
- Version bumped to `0.3.2-rc.1` with `just bump 0.3.2-rc.1`.
- `CHANGELOG.md` and `docs/docs/changelog.md` now include `0.3.2-rc.1` release notes.
- `just release-check` passed after updating web/docs PostCSS overrides and lockfiles to avoid the `postcss <8.5.10` audit advisory.
- Remaining RC gates are manual: smoke checklist, Windows contributor verification, and AMD AV1 real-hardware validation / conservative support wording.

## Scope

This document now tracks two layers of work:

1. Completed / active foundational work from the earlier shortlist
2. The next selected queue you asked me to plan:
   - `INT-3` ntfy notification target
   - `AUTO-2` Quiet hours for notifications
   - `MIG-3` Standardize API Error Schema
   - `IMPR-1` Astro Content Collections for help content
   - `PERF-1` FFprobe result cache
   - `INT-1` Sonarr / Radarr webhook ingress

The earlier shortlist is still relevant as context because parts of it are already implemented or in flight (`UX-1`, `F-4`, `OBS-1`), but the primary forward plan below is for the six items above.

Note: there is only one repo-level `ideas.md` in this workspace right now.

## Selected Queue

These are the items explicitly selected for the next planning horizon:

1. `INT-3` ntfy notification target
2. `AUTO-2` Quiet hours for notifications
3. `MIG-3` Standardize API Error Schema
4. `IMPR-1` Astro Content Collections for help content
5. `PERF-1` FFprobe result cache
6. `INT-1` Sonarr / Radarr webhook ingress

## Recommended Order For The Selected Queue

Recommended implementation order:

1. `MIG-3` Standardize API Error Schema
2. `INT-3` ntfy notification target
3. `AUTO-2` Quiet hours for notifications
4. `INT-1` Sonarr / Radarr webhook ingress
5. `PERF-1` FFprobe result cache
6. `IMPR-1` Astro Content Collections for help content

Why this order:

- `MIG-3` should come first because it improves every API-facing feature that follows, especially notification testing, webhook ingress, and future operator/debugging flows.
- `INT-3` and `AUTO-2` both sit in the notification subsystem, so they share context and are efficient to batch together.
- `INT-1` benefits from cleaner error responses and is the larger external API integration.
- `PERF-1` is more invasive in the backend/media path, so it is safer after the API/integration surfaces are settled a bit.
- `IMPR-1` is useful but non-blocking, so it should trail the higher-leverage backend work.

## Dependencies Between Selected Items

- `MIG-3` improves the ergonomics of:
  - `INT-3` notification test/save errors
  - `AUTO-2` dispatch/validation failures
  - `INT-1` webhook auth/payload validation failures
- `INT-3` and `AUTO-2` share:
  - notification target config shapes
  - delivery pipeline hooks
  - settings UI work in `NotificationSettings.tsx`
- `INT-1` is mostly independent of the notification work, but it benefits from a consistent error contract and from the same “small typed JSON payload” discipline.
- `PERF-1` is independent of the integrations, but it should be planned together with the eventual `PERF-3` directory-scan cache so the two caches do not drift in incompatible ways.
- `IMPR-1` is frontend/docs-local and can be done in parallel with backend work if needed.

## Selected Item Plans

### `MIG-3` Standardize API Error Schema

Current fit:

- Very high.
- The repo still returns mixed plain-text and JSON error shapes.
- Recent audit work already proved this area is worth tightening.

V1:

- Define one shared JSON error response shape, for example:
  - `error`
  - `detail`
  - `code`
- Start with high-traffic endpoints:
  - `/api/jobs/table`
  - `/api/settings/system`
  - `/api/settings/notifications/test`
  - `/api/webhooks/arr` when it lands
- Update frontend API error parsing only where needed to preserve backward compatibility.

V2:

- Add request correlation / request id.
- Roll the schema through the rest of `src/server/`.
- Normalize auth and middleware errors under the same contract.

Stretch:

- Version the API error payload in docs/OpenAPI.
- Give common DB/config/auth failures stable machine codes.

Assessment:

- Best foundation item in this selected queue.

### `INT-3` ntfy Notification Target

Current fit:

- Very high.
- The notification system already supports multiple provider-specific target types and has SSRF-safe delivery plumbing.

V1:

- Add `ntfy` target type.
- Config fields:
  - `server_url`
  - `topic`
  - optional `access_token`
- Deliver through the existing notification manager and retry path.
- Add UI support in `NotificationSettings.tsx`.

V2:

- Optional priority/tags/title fields.
- Better provider-specific test message formatting.

Stretch:

- Topic templates by event type.
- Attachment/link enrichments if ntfy usage proves strong.

Assessment:

- Small, high-value, and should move quickly.

### `AUTO-2` Quiet Hours For Notifications

Current fit:

- Good.
- It reuses the notification pipeline, but there is a subtle scheduling/timezone concern.

V1:

- Add config for quiet-hours start/end.
- Suppress non-critical notification sends inside the quiet-hours window.
- Keep failure notifications immediate by default.

V2:

- Buffer suppressed notifications.
- Send one flush/summary message when the quiet period ends.
- Add per-target overrides.

Stretch:

- Different quiet-hour policies by target type.
- Weekend-only or schedule-window-aware behavior.

Assessment:

- Good follow-on immediately after `INT-3`.

### `INT-1` Sonarr / Radarr Webhook Ingress

Current fit:

- Good.
- The app already has enqueue-by-path, which reduces the core integration risk.

V1:

- Add `POST /api/webhooks/arr`.
- Accept Sonarr/Radarr `Download` payloads.
- Add path-prefix translation config for containerized setups.
- Authenticate with a dedicated token scope.
- Reuse enqueue-by-path.

V2:

- Idempotency guard for duplicate delivery.
- More structured logging and response diagnostics.
- Better docs/examples for Sonarr/Radarr setup.

Stretch:

- Support additional Arr-family integrations if the payload patterns line up.

Assessment:

- High-value integration, but larger than `INT-3`.

### `PERF-1` FFprobe Result Cache

Current fit:

- Strong.
- The analyzer path is a natural cache boundary and the idea is still one of the best backend performance wins.

V1:

- Add additive `media_probe_cache` table.
- Key by:
  - path
  - mtime
  - size
  - ffmpeg/ffprobe version
- Read-through/write-through cache in analyzer.
- Add a simple clear-cache operator action later if needed.

V2:

- Metrics around hit/miss rate.
- Smarter invalidation or pruning.

Stretch:

- Coordinate with `PERF-3` so scan pruning and probe reuse reinforce each other.

Assessment:

- Best selected performance item.

### `IMPR-1` Astro Content Collections For Help Content

Current fit:

- Good.
- Low-risk frontend cleanup and content-ops improvement.

V1:

- Add `web/src/content/help/`.
- Define Zod-backed collection schema.
- Move one or two tooltip/help surfaces onto collections.

V2:

- Expand to more codec/preset help.
- Use collections to power richer inline docs/help panels.

Stretch:

- Localizable help content.
- Shared markdown-driven docs snippets across pages.

Assessment:

- Useful, but should trail the more leverage-heavy backend items.

## What The Codebase Already Gives Us

- The jobs UI already centralizes filter/search/sort state in `web/src/components/JobManager.tsx`.
- The backend already has a `ui_preferences` table and `/api/ui/preferences`, even though it currently only stores `active_theme_id`.
- Manual single-path enqueue already exists and reuses the same dedupe/output logic as scan-driven intake.
- Watch-folder assignments are path-based today, not foreign-keyed into `jobs`.
- Analysis batches only pick `queued` / `failed` jobs that do **not** already have a `decisions` row.
- The scanner is still a full tree walk built on `WalkDir`; there is no directory-state cache yet.
- The codebase does not currently use the `metrics` crate, so Prometheus instrumentation should start with the lowest-friction option.

## External Findings

### Arr webhook shape

From the official Sonarr and Radarr source:

- Sonarr `BuildOnDownloadPayload` emits `EventType=Download` with `Series`, `Episodes`, `EpisodeFile`, `IsUpgrade`, `DownloadClient`, `DownloadId`, `Release`, and `CustomFormatInfo`.
- Radarr `BuildOnDownloadPayload` emits `EventType=Download` with `Movie`, `RemoteMovie`, `MovieFile`, `IsUpgrade`, `DownloadClient`, `DownloadId`, `Release`, and `CustomFormatInfo`.
- Both payload families include a final imported path and a source path on the file object, which is enough to map the webhook into Alchemist’s existing enqueue-by-path flow.

### Prometheus implementation choice

From the official Prometheus format docs and Rust crate docs:

- Prometheus scrape output wants the standard text exposition format with `text/plain; version=0.0.4`.
- The `prometheus` crate is the cheapest fit for this codebase because it supports an explicit `Registry`, `gather`, and `TextEncoder` without adopting a new app-wide instrumentation system.
- `metrics-exporter-prometheus` is stronger if we want long-term native instrumentation, histograms, and its own listener/upkeep model, but it assumes `metrics`-based instrumentation that Alchemist does not currently have.

### Homepage widget path

From Homepage’s official docs:

- Homepage already supports a `customapi` service widget that can render arbitrary JSON with field mappings.
- That means `INT-4` does not need an upstream Homepage PR to become useful. Alchemist can ship `/api/widget/summary` and docs first, then decide later whether a native Homepage widget PR is worth it.

## Shortlist Feasibility

### 1. `UX-1` Saved Filter Views

Current fit:

- Very high.
- The state already exists in one place in `JobManager.tsx`.
- The backend already has a preference surface that can be extended.

Key correction to the original idea:

- V1 should **not** write saved views into the TOML config.
- This is UI state, and the existing `ui_preferences` path is a better fit than mutating config on every save.

V1:

- Persist `saved_job_views` through the existing generic UI preference helpers / `ui_preferences` table instead of writing into TOML.
- Ship three built-ins that the current filter model can actually express, for example `Recent failures`, `Queued`, and `Recent completions`.
- Render chips above the jobs table and apply saved search/tab/sort state.

V2:

- Save current view.
- Rename/delete views.
- Reorder views.
- Sync view state into the querystring for shareable links.

Stretch:

- Per-user views if auth ever becomes multi-user.
- Tie into keyboard shortcuts.

Assessment:

- Best first implementation target from the shortlist.

### 2. `F-4` Batch Re-analyze Watch Folder

Current fit:

- Good, but slightly trickier than the idea text suggests.
- Jobs do not store `watch_dir_id`, so the watch-folder relationship has to be resolved by input-path prefix.
- Re-analysis cannot just flip status. It also needs to remove or bypass existing decisions, because analysis batches exclude jobs that already have a `decisions` row.

V1:

- Add `POST /api/watch-dirs/:id/reanalyze`.
- Resolve the selected watch dir path.
- Select non-archived jobs whose `input_path` is equal to or nested under that root.
- Delete their decision rows.
- Reset eligible rows back to `queued` with `progress = 0`.
- Trigger `analyze_pending_jobs()`.
- Return `{ count, watch_dir, path }`.

V2:

- Show a pre-flight count in the UI.
- Add a confirmation modal.
- Preserve and surface previous-vs-new decision diffs.

Stretch:

- Re-analyze canonical setup roots too, not just extra watch dirs.
- Background task progress stream.
- Dry-run mode that only reports how many rows would be touched.

Assessment:

- Good second target.
- This is small enough to ship early, but it needs a proper DB helper rather than ad hoc path handling in the handler.

### 3. `OBS-1` Prometheus `/metrics`

Current fit:

- Good, if we keep V1 narrow.
- The app already has enough stats surfaces to populate a useful first scrape without touching the encoding pipeline.

Implementation choice:

- Use the `prometheus` crate for V1.
- Avoid `metrics-exporter-prometheus` until there is a concrete need for a recorder/upkeep-based instrumentation layer.

V1:

- Add `metrics.enabled` and a bind/path decision in config.
- Expose:
  - queue depth by state
  - completed jobs total
  - bytes saved total
- Encode with `TextEncoder`.

V2:

- Add failure counters by code.
- Add encoder/backend counters.
- Add encode duration histograms.

Stretch:

- Separate metrics listener.
- Optional IP allowlist or loopback-only bind.
- OpenMetrics or richer histogram support later.

Assessment:

- Strong candidate for milestone 2.
- The biggest risk is over-scoping the first pass.

### 4. `INT-1` Sonarr / Radarr Webhook Ingress

Current fit:

- Good, because the app already supports enqueue-by-absolute-path.
- The webhook can map directly into that flow instead of inventing new queue semantics.

Real work hidden by the idea text:

- The current API token model only supports `read_only` and `full_access`.
- If we want a dedicated `arr_webhook` capability, that means enum, migration, middleware, and UI work.
- Docker path translation is a real requirement for many self-hosted deployments.

Recommended shape:

- Keep the idea’s security intent.
- Add a third token scope dedicated to `/api/webhooks/arr`.

V1:

- Add `arr_webhook` token access level.
- Add `POST /api/webhooks/arr`.
- Accept Sonarr/Radarr `Download` events only.
- Resolve path in this order:
  - final library path (`EpisodeFile.Path` / `MovieFile.Path`)
  - fallback source path
- Add optional prefix mapping config: list of `{ from, to }`.
- Reuse the existing enqueue-by-path logic.

V2:

- Add idempotency guard for repeated webhook delivery.
- Better structured logs.
- Support `Grab` as an optional pre-enqueue hint if we want it.

Stretch:

- Webhook replay test fixture endpoint in dev/test.
- Docs with copy-paste Sonarr/Radarr configuration examples.
- Optional Prowlarr/Lidarr-style reuse if the payload family is close enough.

Assessment:

- High-value ecosystem feature.
- Better after the smaller UI/backend wins land.

### 5. `PERF-3` Incremental Mtime-Based Scan

Current fit:

- Valuable, but the original idea is under-scoped for the current scanner implementation.
- The scanner does a full recursive `WalkDir` and only knows about root directories, not directory-level cache state.

Key correction to the original idea:

- A single `last_scanned_at` on the watch-root row is not enough to prune deep trees safely.
- To skip descending into unchanged directories, Alchemist needs **per-directory** cached scan state.

V1 foundation:

- Add a `directory_scan_cache` table keyed by canonical directory path.
- Store at least:
  - directory path
  - root/watch path
  - last seen directory mtime
  - last scanned at
- Replace the naive recursive tree walk with a scanner that can prune children when the directory mtime has not changed.
- Add a `force full scan` escape hatch.

V2:

- Add cache cleanup for deleted directories.
- Integrate watcher events as cache invalidation hints.
- Pair with `PERF-1` ffprobe result cache for multiplicative wins.

Stretch:

- Per-root scan timing metrics.
- Automatic fallback disable on filesystems with unreliable directory mtimes.
- Network-mount compatibility mode.

Assessment:

- Probably the most valuable long-term performance item here.
- Also the most invasive. It should come after smaller wins unless performance pressure is urgent.

## Recommended Build Order

1. `UX-1` Saved filter views
2. `F-4` Batch re-analyze watch folder
3. `OBS-1` Prometheus `/metrics` v1
4. `INT-1` Sonarr / Radarr webhook ingress
5. `PERF-3` Incremental scan foundation

Why this order:

- It starts with the lowest-risk work that already fits the current architecture.
- It lands a useful operator/backend win before the larger external integration.
- It saves the invasive scanner rewrite until the surrounding queue and observability surfaces are in better shape.

## Milestone Plan

### Milestone A

- `UX-1` saved views v1 using `ui_preferences`
- `F-4` watch-folder re-analyze v1

Goal:

- Two user-visible wins without schema churn beyond small preference additions and focused DB helpers.

### Milestone B

- `OBS-1` `/metrics` v1

Goal:

- Basic Prometheus scrapeability with minimal architectural commitment.

### Milestone C

- `INT-1` Arr webhook ingress v1

Goal:

- Immediate interoperability with Sonarr/Radarr download/import events.

### Milestone D

- `PERF-3` incremental scan cache foundation
- Optional pairing with `PERF-1` ffprobe cache if the scan bench still points strongly there

Goal:

- Turn the scan path into something that scales to large libraries without blindly walking every subtree every time.

## Additional High-Value Ideas From `ideas.md`

These are the next-best candidates after the initial shortlist:

### `MIG-3` Standardize API Error Schema

- Strong cross-cutting foundation.
- Helps the frontend, future webhook consumers, and `/metrics`/operator surfaces present failures consistently.

### `OP-3` Guided Restore From Backup Snapshot

- Pairs naturally with the backup work that already exists.
- High operator value and safer disaster recovery story.

### `PERF-1` FFprobe Result Cache

- Probably the best direct companion to `PERF-3`.
- Reduces re-probe overhead even when the tree walk still finds many files.

### `OBS-2` Structured JSON Logging Mode

- Cheap observability win.
- Minimal product risk.

### `INT-3` ntfy Notification Target

- Small integration with disproportionate homelab appeal.
- Notification subsystem is already mature enough for it.

### `F-2` Library Plan Preview In The Web UI

- Great leverage on the existing planner.
- Strong onboarding value.

### `ENC-3` Profile Bake-Off On A Sample Clip

- The new conversion workflow makes this much more realistic than it used to be.
- Strong differentiator for profile tuning.

### `POL-3` Live FFmpeg Log Viewer Refinement

- Leverages existing SSE/log groundwork.
- Small polish item with obvious user payoff.

### `AUTO-1` Rules Engine For Conditional Profile Routing

- Potentially the biggest strategic differentiator in `ideas.md`.
- Also the highest complexity. Keep it later on purpose.

### `IMPR-1` Astro Content Collections For Help Content

- Low product risk.
- Good cleanup item once feature pace slows down slightly.

## Adjacent Note: `INT-4` Homepage Dashboard Widget

This one is worth doing soon, but not as a full native Homepage integration first.

Recommended path:

1. Ship `/api/widget/summary`.
2. Document it against Homepage’s existing `customapi` widget.
3. Only then decide whether an upstream Homepage widget PR is worth the maintenance cost.

## References

- Sonarr webhook download payload builder:
  - `https://raw.githubusercontent.com/Sonarr/Sonarr/develop/src/NzbDrone.Core/Notifications/Webhook/WebhookBase.cs`
- Sonarr webhook import payload shape:
  - `https://raw.githubusercontent.com/Sonarr/Sonarr/develop/src/NzbDrone.Core/Notifications/Webhook/WebhookImportPayload.cs`
- Radarr webhook download payload builder:
  - `https://raw.githubusercontent.com/Radarr/Radarr/develop/src/NzbDrone.Core/Notifications/Webhook/WebhookBase.cs`
- Radarr webhook import payload shape:
  - `https://raw.githubusercontent.com/Radarr/Radarr/develop/src/NzbDrone.Core/Notifications/Webhook/WebhookImportPayload.cs`
- Prometheus exposition format:
  - `https://prometheus.io/docs/instrumenting/exposition_formats/`
- Rust `prometheus` crate:
  - `https://docs.rs/prometheus/latest/prometheus/`
- Rust `metrics-exporter-prometheus` crate:
  - `https://docs.rs/metrics-exporter-prometheus/latest/metrics_exporter_prometheus/`
- Homepage Custom API widget docs:
  - `https://gethomepage.dev/widgets/services/customapi/`
