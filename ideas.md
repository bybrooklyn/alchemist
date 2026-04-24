# Alchemist — Ideas

*Forward-looking ideas for features, UX, integrations, and polish. Bugs go in `audit.md`.*

**Last updated:** 2026-04-23

## Top picks

1. [MIG-3] Standardize API Error Schema — eliminates inconsistent error handling across backend handlers (Axum).
2. [OP-3] Guided backup restore — completes the new backup feature with a safe recovery path for real operator incidents.
3. [IMPR-1] Astro Content Collections for help content — type-safe, Zod-validated codec/preset tooltips.
4. [PERF-3] Incremental mtime-based scan — significantly faster re-scans for huge libraries.
5. [F-4] Batch re-analyze watch folder — force a full library analysis without a full scan.

---

## Features

### [F-1] Undo last encode

**Category:** Features
**Size:** M
**Touches:** backend, frontend, schema

**Problem or gap:**

Alchemist is careful about not overwriting originals until quality checks pass, but once a user *chose* to replace the original there's no UI path back. A self-hoster who realizes "that AV1 encode of Dune looks off" has to dig into filesystem backups. There's no per-job "restore original" button.

**Idea:**

For any Completed job whose `output_path` replaced the original, track the backup/original path (if still present) and expose a "Restore original" action in `JobDetailModal.tsx`. On click: swap the files atomically, update the job row to a new terminal state `Reverted`, emit a `JobEvent::Reverted`. Add a retention setting "Keep original for N days after replace" so restores remain possible during the window.

**First step:**

Add a nullable `original_backup_path` column to `jobs` (additive migration) and populate it whenever Alchemist would delete/replace the original. No UI yet — just prove the data is captured correctly across a real encode.

**Risks / tradeoffs:**

Doubles temporary disk usage during retention window; needs a clear storage-impact warning in Settings.

---

### [F-2] Library plan preview in the web UI

**Category:** Features
**Size:** S
**Touches:** backend, frontend

**Problem or gap:**

`alchemist plan <path> --json` already exists in the CLI, but a new user adding a watch folder has no way to see what Alchemist *would* do before committing. They have to enqueue and babysit, or learn the CLI.

**Idea:**

Add a "Preview" button on `web/src/components/WatchFolders.tsx` (and in the setup wizard). Backend handler reuses the existing planner in dry-run mode and returns a summary: N skip, N remux, N encode, total estimated savings. Show as a compact modal with per-category counts and a collapsible example list (first 20 files). Nothing is enqueued.

**First step:**

Wire a new `POST /api/library/preview` endpoint that takes a path and returns planner decisions without side effects. Test it hitting a real folder.

**Risks / tradeoffs:**

Analysis is expensive; cap preview scope (first N files or 60s timeout) and show "partial preview" clearly.

---

### [F-3] Preset export / import

**Category:** Features
**Size:** S
**Touches:** backend, frontend

**Problem or gap:**

Quality profiles live only in the user's own config. There's no way to share a known-good preset ("h.265 anime, -crf 22, passthrough audio") across instances or with other users on Reddit.

**Idea:**

Add export/import buttons to `QualitySettings.tsx`. Export serializes the selected profile to a versioned JSON blob (includes schema version, codec, rate control, audio rules, stream rules). Import validates the schema and appends as a new profile. Start a `presets/` folder in the docs with a handful of community presets.

**First step:**

Define the preset JSON schema (with `preset_schema_version: 1`) and add a unit test that round-trips a profile through export → import.

**Risks / tradeoffs:**

Preset schemas drift with new options; the version field gates future migrations.

---

### [F-4] Batch re-analyze watch folder

**Category:** Features
**Size:** S
**Touches:** backend, frontend

**Problem or gap:**

If a user updates their library profile (e.g., changes target codec or quality), they have no way to force Alchemist to re-analyze all files in that watch folder immediately. They have to wait for a periodic scan or use the CLI.

**Idea:**

Add a "Re-analyze folder" button to the watch folder list. Backend iterates over all non-archived jobs associated with that watch folder and forces them back into `Analyzing` state (preserving previous decision for diffing if possible).

**First step:**

Add a `POST /api/watch-folders/:id/reanalyze` endpoint and verify it can reset job states for a specific folder.

**Risks / tradeoffs:**

CPU spike if triggered on a 10k+ library; add a confirmation modal with count.

---

## UX

### [UX-1] Saved filter views on the jobs page

**Category:** UX
**Size:** M
**Touches:** frontend, config

**Problem or gap:**

`JobsToolbar` filters are ephemeral — users who regularly check "failed in the last 24h" or "remux candidates not yet acted on" re-type the filter every time. Power users with 10k+ jobs live on the jobs page.

**Idea:**

Add a "Save view" button next to filter inputs. Persist saved views in the user's config (named entries with filter state + sort). Render them as chips across the top of `JobsTable`. Shift+click to rename, right-click to delete. Ship with three built-in views: "Recent failures", "Queued", "Today's completions".

**First step:**

Add a `saved_job_views: Vec<SavedJobView>` field to config and a single built-in view rendered as a chip. Prove persistence before building the save/rename UI.

**Risks / tradeoffs:**

Filter-state schema needs to tolerate added fields — use flat keys with `#[serde(default)]`.

---

### [UX-2] Keyboard shortcuts across the UI

**Category:** UX
**Size:** S
**Touches:** frontend

**Problem or gap:**

Every action in Alchemist requires a mouse click. Power users managing thousands of jobs want j/k row navigation, `/` to focus search, `Esc` to close modals, `.` to open detail on the selected row.

**Idea:**

Introduce a lightweight keyboard map on the jobs page first (`JobManager.tsx`): j/k (next/prev row), Enter (open detail), Esc (close), `/` (focus filter), `p` (toggle pause on selected). A small "?" modal listing shortcuts is discoverable via `?`.

**First step:**

Add a single global key handler for `/` → focus filter input and `?` → shortcuts modal. Ship that and collect feedback before expanding.

**Risks / tradeoffs:**

Must not intercept keys when a text input is focused — use a `useHotkey` wrapper that checks event target.

---

### [UX-3] Search skip explanations

**Category:** UX
**Size:** S
**Touches:** frontend

**Problem or gap:**

Alchemist generates plain-English skip explanations (README flagged as a headline feature). But there's no way to ask the jobs page "show me everything skipped because of HDR metadata" or "everything skipped as already-efficient" across the library.

**Idea:**

Add a free-text search box on `JobsTable` that matches against the explanation field in addition to path/filename. Surface common skip reasons as suggested chips under the search box ("already efficient", "HDR", "subtitle incompatibility").

**First step:**

Expose a `?q=...` query parameter on the existing jobs list endpoint that matches `explanation ILIKE '%q%'`. Wire the search input to it.

**Risks / tradeoffs:**

LIKE on a large explanations column will be slow; plan for an index on a tokenized copy if perf shows up as an issue.

---

### [UX-5] Bulk actions toolbar

**Category:** UX
**Size:** S
**Touches:** frontend

**Problem or gap:**

The jobs page has multi-select checkboxes, but the toolbar only offers "Add file" and "Refresh". There's no way to act on the selection (e.g., "Cancel all 50 selected jobs"). Users have to click the "More" menu on every row.

**Idea:**

When `selected.size > 0`, transform the `JobsToolbar` (or show a floating action bar) with bulk actions: "Cancel Selected", "Restart Selected", "Delete Selected". Wire them to the existing `batch_jobs_handler` in the backend.

**First step:**

Conditional rendering in `JobsToolbar.tsx` that surfaces "Cancel (N)" when items are selected.

**Risks / tradeoffs:**

None.

---

## Integrations

### [INT-1] Sonarr / Radarr webhook ingress

**Category:** Integration
**Size:** M
**Touches:** backend, frontend, docs

**Problem or gap:**

The *arr stack is the dominant library-automation pattern for self-hosters. Today, Alchemist only picks up changes via watch folder scans, so a new episode from Sonarr isn't encoded until the next scan fires. Users bolt on cron/inotify hacks.

**Idea:**

Add an inbound webhook endpoint `POST /api/webhooks/arr` that accepts Sonarr/Radarr's "Download" and "Upgrade" event payloads, maps the file path through a configurable prefix (for Docker path-translation), and immediately enqueues an analyze job. Authenticate via a dedicated API token kind (`arr_webhook`). Document the setup in `docs/` with copy-paste Sonarr config.

**First step:**

Write a handler that accepts the Sonarr test-event JSON and logs it. Validate the payload shape against current Sonarr/Radarr docs before building enqueue logic.

**Risks / tradeoffs:**

Sonarr/Radarr payload schemas evolve — parse defensively with `#[serde(default)]` and log unknown fields rather than failing.

---

### [INT-2] Jellyfin / Plex library-refresh hook

**Category:** Integration
**Size:** S
**Touches:** backend, config

**Problem or gap:**

After Alchemist replaces a file, Jellyfin and Plex don't re-scan until their own schedule runs, so users see stale metadata or wrong runtimes for hours. The fix is a one-shot API call, but Alchemist doesn't make it.

**Idea:**

Add notification-channel-style targets for Jellyfin and Plex in `NotificationSettings.tsx`: URL + API key. On job finalization, POST to the library-refresh endpoint for the affected section. Bundle multiple finalizations into a single refresh call within a 60s window to avoid thrashing.

**First step:**

Ship a Jellyfin-only implementation behind a `jellyfin_refresh` config flag; validate end-to-end on a real Jellyfin before adding Plex.

**Risks / tradeoffs:**

Section mapping — users may have multiple libraries, so the config needs a section ID, not just a server URL.

---

### [INT-3] ntfy notification target

**Category:** Integration
**Size:** S
**Touches:** backend, frontend, config

**Problem or gap:**

Alchemist supports Discord, Gotify, Telegram, email, and webhook. ntfy.sh is the de facto lightweight homelab push target now and is visibly missing.

**Idea:**

Add an `Ntfy` variant to the notification target enum. Config fields: server URL, topic, optional access token for private instances. Mirror Gotify's UI in `NotificationSettings.tsx`. Emit the same event types as other targets.

**First step:**

Implement the POST-to-topic call and verify against a public `ntfy.sh/test-topic`. Reuse the existing notification retry + SSRF protections.

**Risks / tradeoffs:**

None significant — ntfy's API is stable and tiny.

---

### [INT-4] Homepage dashboard widget

**Category:** Integration
**Size:** S
**Touches:** backend

**Problem or gap:**

Homepage (gethomepage.dev) is the default dashboard for self-hosters. Most competing tools have a widget. Alchemist requires users to open the full UI to see queue depth or savings.

**Idea:**

Expose a small read-only `GET /api/widget/summary` (gated by read-only API token) that returns JSON: `{queue, running, completed_today, bytes_saved_total}`. Submit a PR to the Homepage widgets repo. Link from the Alchemist README once merged.

**First step:**

Write the handler using existing stats queries and document the exact JSON shape. Test with a real Homepage instance.

**Risks / tradeoffs:**

The Homepage PR review is the unknown — schema needs to match their widget conventions.

---

## Performance

### [PERF-1] FFprobe result cache

**Category:** Performance
**Size:** M
**Touches:** backend, schema

**Problem or gap:**

Every scan re-probes every file. For a library of tens of thousands of files, most of which haven't changed since the last scan, this is a huge chunk of the scan time — especially on spinning rust. FFprobe is deterministic for a given byte-identical file.

**Idea:**

Add a `media_probe_cache` table keyed by `(path, mtime, size)` storing the serialized `AnalyzerResult`. In the analyzer, check the cache first and reuse on hit; on mtime/size mismatch, re-probe and update. Add a "Clear probe cache" operator action in `SystemSettings.tsx`.

**First step:**

Add the table as an additive migration and the cache read/write in `src/media/analyzer.rs` behind a `probe_cache_enabled: bool` config flag defaulted on. Measure scan time on a 10k file fixture before and after.

**Risks / tradeoffs:**

FFprobe output schema changes across FFmpeg versions — store the FFmpeg version alongside the cached blob and invalidate on mismatch.

---

### [PERF-2] Source-drive job grouping

**Category:** Performance
**Size:** M
**Touches:** backend, config

**Problem or gap:**

With multiple concurrent jobs and watch folders spanning several drives (common for self-hosters with separate movies/TV disks), the queue picks jobs in priority/FIFO order, which makes two HDDs seek against themselves. Actual throughput is much lower than the per-drive ceiling.

**Idea:**

Resolve each enqueued job's source drive via `std::fs::canonicalize` + device ID (dev_t on Unix, volume serial on Windows). In the processor, prefer at most one concurrent job per source device unless Throughput mode is selected.

**First step:**

Add a `source_device` column to `jobs` populated at enqueue time and expose it in the jobs API. Don't change scheduling logic yet — just verify the device IDs look right across Linux, macOS, and Windows.

**Risks / tradeoffs:**

Network mounts and bind mounts can mislead the device check — document the limitation and add a config override.

---

### [PERF-3] Incremental mtime-based scan

**Category:** Performance
**Size:** M
**Touches:** backend, system

**Problem or gap:**

Alchemist scans folders by walking the entire tree. For 100k+ file libraries, even just the `ls` overhead is non-trivial.

**Idea:**

Store the `last_scanned_at` timestamp for each watch folder. During a scan, if a directory's mtime hasn't changed since `last_scanned_at`, skip its children entirely. 

**First step:**

Plumb `last_scanned_at` into the `watch_folders` table and verify it updates after a successful scan.

**Risks / tradeoffs:**

Fails if the OS doesn't propagate child mtime changes to parent (rare on modern FS, but possible on network mounts). Needs a "Force full scan" override.

---

## Polish

### [POL-1] Shift-click bulk select in jobs table

**Category:** Polish
**Size:** S
**Touches:** frontend

**Problem or gap:**

Selecting a range of jobs requires clicking each checkbox individually. Shift-click-to-select-range is the universal expectation.

**Idea:**

On checkbox click, if shift is held and a prior selection exists, select every row between the two. Track the "anchor" row in `JobsTable` state. Reset anchor on filter change.

**First step:**

Add a `lastSelectedIndex` state and extend the onChange handler. Trivial — could be done in an hour.

**Risks / tradeoffs:**

None.

---

### [POL-2] Right-click context menu in jobs table

**Category:** Polish
**Size:** S
**Touches:** frontend

**Problem or gap:**

Per-row actions (pause, cancel, restart, copy path) live in a button column or require opening the detail modal. Power users want a right-click menu, especially on multi-select.

**Idea:**

On row context-menu event, show a small menu with Pause/Resume, Cancel, Restart, Copy input path, Copy FFmpeg command, Open in detail. Reuse the actions from `JobDetailModal.tsx`. Respect the current selection — if multiple rows are selected and the click lands on one of them, the action applies to all.

**First step:**

Build the menu with a single action ("Copy input path") and verify positioning, dismissal on click-outside, and selection behavior.

**Risks / tradeoffs:**

Mobile/touch lacks right-click — keep the detail modal as the primary path, treat right-click as power-user shortcut.

---

### [POL-3] Live FFmpeg log viewer refinement

**Category:** Polish
**Size:** S
**Touches:** frontend, backend

**Problem or gap:**

The current log viewer in `JobDetailModal.tsx` is a static list. For active encodes, users want to see the tail of the FFmpeg output in real-time.

**Idea:**

Stream logs via SSE (already mostly implemented in `sse.rs`) and have the `JobDetailModal` auto-scroll to the bottom of the log list when it's open for an active job. Add a "Follow logs" toggle.

**First step:**

Wire the SSE log stream into the modal's state for the focused job.

**Risks / tradeoffs:**

SSE overhead if many users have modals open; cap at 100 lines for the live view.

---

## Observability

### [OBS-1] Prometheus `/metrics` endpoint

**Category:** Observability
**Size:** M
**Touches:** backend, docs

**Problem or gap:**

Self-hosters run Prometheus + Grafana for almost everything else. Alchemist has no scrapeable metrics, so its status only exists inside its own UI.

**Idea:**

Add `GET /metrics` returning Prometheus text format (via the `prometheus` or `metrics-exporter-prometheus` crate — pick one and name it explicitly in the PR). Expose: queue depth by state, encodes completed total by codec, bytes saved total, encode duration histogram, encoder selection counts, pipeline error counts. Gate behind a `metrics.enabled` config flag (default off) with a `metrics.bind` option for a separate port.

**First step:**

Expose three metrics (queue depth, completed total, bytes saved) behind the flag and scrape locally to confirm the format renders in Prometheus.

**Risks / tradeoffs:**

Adds a dependency — pick a lightweight one and feature-gate at compile time if weight matters.

---

### [OBS-2] Structured JSON logging mode

**Category:** Observability
**Size:** S
**Touches:** backend, config

**Problem or gap:**

Alchemist logs are human-readable but can't be ingested cleanly into Loki, Elasticsearch, or Datadog without regex parsing. Docker users especially want JSON-lines output.

**Idea:**

Add a `log_format = "text" | "json"` config key (and `ALCHEMIST_LOG_FORMAT` env var). When set to `json`, switch the `tracing-subscriber` layer to `tracing_subscriber::fmt::json()`. Document a sample Loki query in `docs/`.

**First step:**

Flip the subscriber based on the env var; run the server and tail the output.

**Risks / tradeoffs:**

None — both formats are already supported by `tracing-subscriber`.

---

### [OBS-3] Failure and skip trends by explanation code

**Category:** Observability
**Size:** M
**Touches:** backend, frontend, docs

**Problem or gap:**

Alchemist now stores structured decision and failure codes, but operators still have to infer systemic issues one job at a time. If a profile update starts tripping `subtitle_incompatible` or a specific encoder backend begins failing all week, the current UI offers no aggregate answer.

**Idea:**

Add a stats surface that groups recent jobs by `decisions.reason_code` and `job_failure_explanations.code`, with counts and simple trends for the last 24h / 7d / 30d. Surface it on the stats page as “Top skip reasons” and “Top failure reasons,” with drill-down links back to filtered jobs.

**First step:**

Add one backend query that returns the top failure codes for the last 7 days, keyed by code and count, and render it as a compact table on the stats page.

**Risks / tradeoffs:**

The value depends on stable code taxonomies, so future code renames need an alias/migration story.

---

## Operator

### [OP-1] One-click SQLite backup + download

**Category:** Operator
**Size:** S
**Touches:** backend, frontend

**Problem or gap:**

Backing up the DB today requires users to know the file path (`ALCHEMIST_DB_PATH`) and SSH into the container. Self-hosters forget until they need the backup.

**Idea:**

Add a "Backup database" button to `SystemSettings.tsx`. Backend uses SQLite's online backup API (via `rusqlite::backup::Backup` or equivalent in sqlx) to snapshot into a timestamped file, then streams it as a download. Also expose via `POST /api/system/backup` with `full_access` token auth. Document in the README.

**First step:**

Write the snapshot-to-temp-file + stream-download handler. Confirm the snapshot is consistent under concurrent writes by running it while an encode is active.

**Risks / tradeoffs:**

Large DBs take time and disk space — stream compressed (gzip) and warn on expected duration.

---

### [OP-2] Pipeline self-test with a known-good sample

**Category:** Operator
**Size:** M
**Touches:** backend, frontend

**Problem or gap:**

When a user's encodes all fail, they don't know if it's their config, hardware, FFmpeg, or a bug. There's no "is the pipeline itself healthy" diagnostic.

**Idea:**

Add a "Run pipeline self-test" button in `SystemStatus.tsx`. Backend ships a tiny (10s, 240p, public-domain) sample embedded via `include_bytes!`, runs it through the full pipeline with the user's current profile, and reports: stage completed, encoder used, wall time, success/failure. Output file is discarded.

**First step:**

Pick and license-verify a sample clip, embed it, and wire a CLI-only `alchemist selftest` subcommand. Ship the UI after the CLI path is stable.

**Risks / tradeoffs:**

Embedding adds binary size (~500KB for a decent sample) — document and justify.

---

### [OP-3] Guided restore from a downloaded backup snapshot

**Category:** Operator
**Size:** M
**Touches:** backend, frontend, docs

**Problem or gap:**

Alchemist now offers one-click backup download, but disaster recovery still requires the operator to stop the service and manually swap SQLite files on disk. That is exactly the kind of maintenance path homelab users avoid until they are already in a stressful “I need this restored now” moment.

**Idea:**

Add a restore workflow in **Settings → Runtime** that accepts an uploaded `.db.gz`, validates that it is a readable Alchemist snapshot, takes a fresh pre-restore backup, then swaps the database in a controlled maintenance window. The UI should show the snapshot’s version/schema metadata before the operator confirms the restore.

**First step:**

Implement a dry-run validation endpoint that accepts a backup upload, decompresses it to a temp file, verifies the SQLite header and current migration level, and returns metadata without mutating the live database.

**Risks / tradeoffs:**

Restore flows are inherently dangerous — require maintenance mode, a fresh automatic safety backup, and an explicit restart step.

---

## Encoding

### [ENC-1] Audio loudness normalization (EBU R128)

**Category:** Encoding
**Size:** M
**Touches:** backend, frontend, config

**Problem or gap:**

Users re-encoding mixed-source libraries (movies + rips + home videos) hit wildly uneven loudness. FFmpeg supports `loudnorm` but there's no UI for it; users have to hand-edit config or shell out.

**Idea:**

Add an "Audio normalization" section in `TranscodeSettings.tsx` with a toggle and EBU R128 target parameters (integrated LUFS, LRA, true peak). On encode, insert the `loudnorm` filter with a two-pass analyze-then-apply flow (already standard FFmpeg practice).

**First step:**

Plumb the config key and add the filter to the `FFmpegCommandBuilder` behind an off-by-default flag. Unit-test the arg output.

**Risks / tradeoffs:**

Two-pass analysis roughly doubles audio processing time — disclose in the tooltip.

---

### [ENC-2] Chapter preservation verification

**Category:** Encoding
**Size:** S
**Touches:** backend

**Problem or gap:**

Chapter markers silently disappear on some codec/container combinations (especially re-muxing into formats without chapter support). Users notice weeks later when a movie has no chapters.

**Idea:**

In the analyzer, record the source chapter count. In the finalizer, count chapters in the output via ffprobe. If the output has fewer chapters than the source and the source had any, attach a warning to the job (new `JobWarning` row type, additive) and surface it in the detail modal.

**First step:**

Record source chapter count on the existing analyzer result struct and log a warning when the delta is non-zero — no DB changes yet. Confirm the detection works across MKV/MP4.

**Risks / tradeoffs:**

Some container conversions deliberately drop chapters (e.g., audio-only output) — classify those as expected to avoid false warnings.

---

### [ENC-3] Profile bake-off on a short sample clip

**Category:** Encoding
**Size:** M
**Touches:** backend, frontend, docs

**Problem or gap:**

Tuning a library profile is still a leap of faith. Operators can preview a single convert job, but they cannot easily answer “What does AV1 CRF 30 look like versus HEVC CRF 24 on this exact kind of content?” without running full encodes or dropping to hand-written `ffmpeg` commands.

**Idea:**

Extend the Convert workflow with a “sample bake-off” mode: choose a short clip window from a source file, run two or three profile variants against the same segment, then compare output size, encode speed, VMAF, and representative thumbnails side by side. This turns profile tuning into an interactive workflow instead of repeated full-library experiments.

**First step:**

Add a backend endpoint that transcodes a bounded clip (for example 30 seconds) with one selected profile and returns artifact metadata plus a preview thumbnail; prove the single-profile slice first before adding side-by-side comparison.

**Risks / tradeoffs:**

Short clips can mislead users on content with highly variable complexity, so the UI needs to frame the result as a sample, not a guarantee.

---

## Automation

### [AUTO-1] Rules engine — conditional profile routing

**Category:** Automation
**Size:** L
**Touches:** backend, frontend, schema, config

**Problem or gap:**

Today, a watch folder maps to exactly one library profile. Real homelabs want "if path contains `/Anime/` and source is x264 ≥ 10Mbps, use profile `anime-av1-cpu`; else profile `default-hevc-nvenc`". Users fake this with separate watch folders.

**Idea:**

Add a rules table and a simple rule editor in settings: ordered list of rules, each with conditions (path glob, codec match, resolution threshold, bitrate threshold, source profile) and an action (set profile, skip, set priority). First matching rule wins. Evaluate during planning; fall back to the watch folder's default profile if no rule matches.

**First step:**

Design the rule JSON schema and prototype a single condition type (path glob) end-to-end: schema → planner → UI. Ship the narrow version and grow it from there.

**Risks / tradeoffs:**

Rule engines accumulate complexity — keep the condition grammar small and explicitly extensible with versioning.

---

### [AUTO-2] Quiet hours for notifications

**Category:** Automation
**Size:** S
**Touches:** backend, frontend, config

**Problem or gap:**

Alchemist can notify on every finished job. For users with long queues and real-time notification targets (Discord, ntfy), that's spammy overnight.

**Idea:**

Add a "Quiet hours" section to `NotificationSettings.tsx`: start/end times in the user's configured timezone. During quiet hours, buffer non-critical events and flush them as a single summary when the window ends. Critical events (job failure, engine error) still notify immediately unless the target explicitly opts out.

**First step:**

Add the config fields and short-circuit notification dispatch within the window. Buffering/summary comes in a second pass.

**Risks / tradeoffs:**

Timezone handling — reuse whatever zone the scheduler already uses so the two features agree.

---

## Migration and Improvements

### [MIG-3] Standardize API Error Schema

**Category:** Migration
**Size:** S
**Touches:** backend

**Problem or gap:**

Currently, Axum handlers return various error shapes: some return a plain string, others return JSON, some swallow DB errors into 401s (Audit P2-16). This makes it hard for the frontend and CLI to reliably report what went wrong.

**Idea:**

Define a global `AlchemistError` JSON response: `{ "error": "Internal Server Error", "detail": "SQLite busy", "code": "DB_LOCKED", "request_id": "uuid" }`. Update `AppState` and all handlers to use this via an `IntoResponse` implementation.

**First step:**

Define the struct and apply it to one high-traffic endpoint (e.g., `/api/jobs`).

**Risks / tradeoffs:**

Breaking change for CLI consumers; version the API or keep a legacy compatibility shim.

### [IMPR-1] Astro Content Collections for help content

**Category:** Improvement
**Size:** S
**Touches:** frontend

**Problem or gap:**

Codec and quality preset help text (the "plain English explanations") are hardcoded in `JobExplanations.ts` or scattered in components. This makes them hard to update or localize.

**Idea:**

Use Astro 5 Content Collections to manage a `help/` directory of markdown files. Use Zod to validate fields like `codec`, `preset`, and `explanation_slug`. This makes the "Intelligence" page and tooltips data-driven.

**First step:**

Create `web/src/content/help/nvenc.md` and use `getCollection` in the settings page.

**Risks / tradeoffs:**

Adds build-time overhead for Astro; trivial for this scale.
