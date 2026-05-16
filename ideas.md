# Alchemist — Ideas

*Forward-looking ideas for features, UX, integrations, and polish. Bugs go in `audit.md`.*

**Last updated:** 2026-05-15

## Top picks

1. [ENC-5] Sample-based VMAF pre-flight quality gate — bail on a bad profile before spending the full GPU hours, not after.
2. [F-12] Smart crop / black-bar removal — reclaims bitrate currently spent encoding solid black letterbox bars.
3. [POL-7] Human-readable FFmpeg error summaries — turns cryptic stderr dumps into actionable failure badges.
4. [INT-9] Radarr/Sonarr tag synchronization — stops the ARRs from re-downloading files Alchemist already upgraded.
5. [AUTO-5] Scheduled recurring library rescan — cheap with incremental scan; closes the watcher's coverage gaps.

*Earlier picks still relevant: [OP-6] redacted diagnostics bundle, [OP-3] guided restore, [AUTO-3] disk-space guardrails.*

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

### [F-6] In-app auto-update

**Category:** Features
**Size:** L
**Touches:** backend, frontend, docs

**Problem or gap:**

Binary users (non-Docker) have to manually download releases, stop the service, and replace the executable. This friction leads to users running stale versions with known bugs. Docker users have Watchtower/Ouroboros, but native users have no "Update" button.

**Idea:**

Introduce an "Update available" indicator in the UI (already partially researched in `get_system_update_handler`). Add a "Download and Update" button that: 1) Downloads the platform-specific release asset from GitHub, 2) Verifies the GPG signature/checksum, 3) Performs an atomic swap using `self_replace` (Rust crate), and 4) Gracefully restarts the process.

**First step:**

Implement a CLI-only `alchemist system update --check` that reports if a newer version is available and lists the download URL.
**Risks / tradeoffs:**

Permissions — on Windows and some Linux setups, the binary might be in a read-only directory (`/usr/bin`), requiring a clear error message or sudo-elevation helper.

### [F-7] Headless / CLI-only Mode

**Category:** Features
**Size:** M
**Touches:** backend, config

**Problem or gap:**

Alchemist currently assumes a WebUI-first workflow (Setup Wizard → Watch Folders). Advanced users or those running in purely automated environments want to bypass the browser entirely and control everything via flags.

**Idea:**

Allow comprehensive configuration via CLI flags that override `config.toml`. Support flags for: `--codec`, `--append`, `--allow-cpu-encoding`, `--username`, `--password`, `--directory`, `--output-directory`, `--port`, and `--mode` (encode/scan). If these flags are present, the app can skip the setup wizard and start immediately in the requested mode.

**First step:**

Update `src/main.rs` to parse these new flags using `clap` and inject them into the `Config` struct before the server starts.

**Risks / tradeoffs:**

Flag explosion — keep the flag surface strictly aligned with the most critical `config.toml` keys.

---

### [F-8] Tauri / Desktop wrapper

**Category:** Features
**Size:** XL
**Touches:** backend, frontend, build

**Problem or gap:**

Running Alchemist as a background binary + browser tab feels "server-like" even when running locally on a workstation. Some users prefer a self-contained desktop application.

**Idea:**

Use Tauri to wrap the Alchemist backend and Astro frontend into a native desktop app. This provides a system tray icon, native notifications, and a dedicated window, making it feel like a local tool rather than a hosted service.

**First step:**

Initialize a Tauri project in the root and configure it to bundle the `target/release/alchemist` binary as a sidecar.

**Risks / tradeoffs:**

Increases build complexity and artifact size.

---

### [F-9] Plugin / Extension System

**Category:** Features
**Size:** XL
**Touches:** backend, architecture

**Problem or gap:**

As Alchemist grows, users will want specialized integrations (e.g., with Helios or other niche media tools) that don't necessarily belong in the core binary.

**Idea:**

Implement a lightweight plugin system (possibly using WebAssembly or a simple gRPC/Unix-socket-based sidecar architecture). Plugins could hook into job lifecycle events (e.g., `pre-analyze`, `post-finalize`) to perform custom metadata tagging, file movement, or external API calls.

**First step:**

Define a `Plugin` trait in Rust and implement a single "Internal Plugin" that handles the current notification logic to prove the abstraction.

**Risks / tradeoffs:**

Major architectural complexity; must ensure plugins cannot crash the main pipeline or leak secrets.

---

### [F-10] Native SwiftUI macOS Client

**Category:** Features
**Size:** XL
**Touches:** backend, frontend, build (Apple specific)

**Problem or gap:**

macOS users value native-feeling, high-performance apps that integrate with the system's design language. The WebUI, while functional, lacks the polish of a native SwiftUI app.

**Idea:**

Develop a native SwiftUI client for macOS. Option A: Build the Rust core directly into the Swift binary using `uniffi` or `swift-bridge`. Option B: Keep the Rust binary separate but provide a SwiftUI frontend that communicates with the Alchemist API (with an option to disable the WebUI).

**First step:**

Create a minimal SwiftUI prototype that can list jobs by calling the existing local Alchemist API.

**Risks / tradeoffs:**

Platform-specific maintenance burden.

---

### [F-11] Image-based subtitle OCR to text formats

**Category:** Features
**Size:** L
**Touches:** backend, pipeline, schema

**Problem or gap:**

Many library files carry image-based subtitles (PGS/HDMV in Blu-ray rips, VobSub in DVD rips). Web and mobile players can't render those without a server-side subtitle burn or transcode — exactly the downstream transcodes a self-hoster runs Alchemist to avoid. Alchemist currently copies subtitle streams through untouched.

**Idea:**

During analysis, flag image-subtitle streams. Offer an opt-in "OCR image subtitles to SRT" step that extracts each PGS/VobSub track, runs it through an OCR pass, and muxes the resulting text subtitle alongside (not replacing) the original. The planner records an `OcrSubtitle` sub-action so the job detail explains what happened.

**First step:**

Prototype the extraction + OCR of a single PGS track outside the pipeline (a standalone function + test against a sample file) to measure accuracy and runtime before wiring it into `planner.rs`.

**Risks / tradeoffs:**

OCR adds a heavy dependency and is imperfect; keep it opt-in, never delete the original image track, and surface a confidence note.

---

### [F-12] Smart crop / black-bar removal

**Category:** Features
**Size:** M
**Touches:** backend, pipeline, ffmpeg

**Problem or gap:**

Films mastered at 2.39:1 inside a 16:9 container spend real bitrate encoding solid black letterbox bars. Alchemist never detects or removes them, so every encode of such a title wastes space on black pixels.

**Idea:**

Add an analyzer step that runs `ffmpeg cropdetect` over a few sampled segments, takes the stable consensus crop rectangle, and — when the user opts in — applies a `crop` filter to the encode. The decision explanation records the detected geometry so users can see why dimensions changed.

**First step:**

Add a `cropdetect` sampling helper in `analyzer.rs` that returns `Option<CropRect>` for a path, with a unit test asserting a known letterboxed sample yields the expected rectangle. No encode wiring yet.

**Risks / tradeoffs:**

Variable-aspect content (mixed scope/flat) can be cropped wrong; require a stable detection across multiple samples and keep it opt-in per profile.

---

### [F-13] Per-title complexity-adaptive bitrate

**Category:** Features
**Size:** L
**Touches:** backend, planner, ffmpeg

**Problem or gap:**

The planner picks CRF/quality from resolution and a fixed BPP heuristic. A grainy live-action film and a flat animated show at the same resolution get treated alike — the first comes out soft, the second wastes bitrate.

**Idea:**

Run a fast complexity probe (sampled scene-cut density + spatial/temporal information, or a quick CRF test encode of a short slice) and nudge the chosen CRF up or down within a bounded range. Record the measured complexity and the adjustment in the decision explanation.

**First step:**

Add a `complexity_probe(path) -> ComplexityScore` function with a test over two contrasting samples; log what CRF delta it *would* apply without changing live encodes yet.

**Risks / tradeoffs:**

Adds probe time per job; keep the adjustment range small and deterministic so results stay reproducible (a binding design constraint).

---

### [F-14] Chunked distributed encoding (swarm mode)

**Category:** Features
**Size:** L
**Touches:** backend, orchestrator, schema, API

**Problem or gap:**

A single 4K REMUX can take hours on one GPU. Homelabbers often have a second machine with an idle GPU but no way to put it to work — Alchemist is strictly single-node.

**Idea:**

Split a job into keyframe-aligned segments, hand segments to registered worker nodes over an authenticated API, encode in parallel, then concatenate and verify. Workers are stateless; the coordinator owns the queue and the resume bookkeeping (which already exists for segment resume).

**First step:**

Define the worker-node registration + segment-claim API surface and a `segments` schema addition; build the local-only path first (coordinator hands segments to its own worker pool) to prove concat + verify before any network protocol.

**Risks / tradeoffs:**

Large surface area, network trust boundary, and concat artifacts at segment seams. Reuse the existing segment-resume machinery; ship local-parallel first, networked second.

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

### [UX-6] Mobile-optimized "Active Now" mini-dashboard

**Category:** UX
**Size:** S
**Touches:** frontend

**Problem or gap:**

The current Dashboard and Jobs pages are heavy and designed for desktop. Checking progress on a phone (e.g., "Is that 4K movie done yet?") involves a lot of horizontal scrolling and tiny text.

**Idea:**

Create a dedicated mobile-first view (or a responsive variant of the main dashboard) that focuses exclusively on *active* jobs: a single vertical list of progress bars, "Time remaining" estimates, and a big "Pause/Resume" toggle. Access via a bottom-nav icon or a "Mobile View" toggle in the sidebar.

**First step:**

Create a CSS media query in `Dashboard.tsx` that hides the stats cards and expands the "Active Jobs" list to full width on screens < 768px.
**Risks / tradeoffs:**

Maintaining two UI layouts — keep the mobile view as a subset of existing data to minimize state duplication.

### [UX-7] Interactive CLI Setup Wizard

**Category:** UX
**Size:** S
**Touches:** backend

**Problem or gap:**

The first-run experience currently *requires* a browser. If Alchemist is running on a headless server without an easy way to tunnel port 3000 initially, the user is stuck.

**Idea:**

Implement a `--wizard` CLI flag that walks the user through the same setup steps as the WebUI (admin user creation, default library paths, telemetry opt-in) directly in the terminal using a crate like `dialoguer`.

**First step:**

Build a minimal prototype of the CLI wizard that only collects the admin username and password.

**Risks / tradeoffs:**

None.

---

### [UX-8] Persistent Theme in Config

**Category:** UX
**Size:** S
**Touches:** backend, frontend, config

**Problem or gap:**

The current theme selection (light/dark) is stored in `localStorage`. This means the theme resets if the user clears their browser data or switches devices, and it can cause a "flash of wrong theme" before the JS loads.

**Idea:**

Move the theme preference into the `config.toml` file. The backend can then inject the correct CSS class directly into the initial HTML (SSR), ensuring a perfect theme match from the first byte and persistence across all of the user's devices.

**First step:**

Add a `ui_theme` field to the `SystemConfig` struct and expose it via the settings API.

**Risks / tradeoffs:**

Requires a database/config write for a purely visual preference.

---

### [UX-9] Settings impact summary before save

**Category:** UX / Interface
**Size:** S
**Touches:** frontend, backend

**Problem or gap:**

Settings pages can change behavior that affects future scans, watch folders, queued jobs, or notifications, but the save flow rarely explains the blast radius. A self-hoster changing output strategy or quality settings wants to know whether active jobs are affected, which folders inherit the setting, and whether the change is future-only.

**Idea:**

Add a compact "Impact" panel to high-risk settings pages such as `FileSettings.tsx`, `TranscodeSettings.tsx`, `QualitySettings.tsx`, and `WatchFolders.tsx`. The panel summarizes affected watch folders, whether active jobs are untouched, whether queued jobs need re-analysis, and any disk or quality risk. Start with client-side summaries from the existing settings bundle, then add backend-calculated impact for more complex changes.

**First step:**

Implement the panel for `FileSettings.tsx` only, using existing settings data to show whether the current save changes output naming, output root, or replace behavior.

**Risks / tradeoffs:**

An inaccurate impact summary is worse than none; keep the first version conservative and explicit about what it does not know.

---

### [UX-10] "Storage reclaimed" impact widget

**Category:** UX
**Size:** S
**Touches:** frontend

**Problem or gap:**

The Statistics page shows raw byte counts. The single most motivating number in the app — total space saved — lands as an abstract "2.4 TB" with no felt sense of scale.

**Idea:**

Add a compact dashboard widget that translates cumulative savings into tangible equivalents the user picks the framing for: "≈ 610 more 1080p movies", "≈ 3.1 months of a 4K stream", or a simple cost-per-TB estimate. Pure presentation over `get_aggregated_stats` data already on hand.

**First step:**

Add the widget to `StatsCharts.tsx` with one hardcoded equivalence (movies-at-4GB) and verify it reads the existing savings total.

**Risks / tradeoffs:**

Equivalences are rough; label them "approximate" and keep the raw number visible alongside.

---

### [UX-11] Library bitrate-inefficiency heatmap

**Category:** UX
**Size:** M
**Touches:** backend, frontend

**Problem or gap:**

A user with a large library has no way to see *where* the worst offenders live. The Intelligence page lists recommendations but not a spatial sense of which folders/shows hold the most reclaimable space.

**Idea:**

Render a treemap (or sortable folder table) keyed by reclaimable bytes — each node sized by current footprint and shaded by estimated savings ratio from the planner's dry-run. Clicking a node pre-filters the jobs view or enqueues that subtree.

**First step:**

Add a backend aggregation that returns, per top-level library subfolder, `{ file_count, total_bytes, estimated_savings_bytes }`, and render it as a plain sorted table before attempting the treemap.

**Risks / tradeoffs:**

Computing estimated savings library-wide is expensive — reuse cached probe/plan data and bound the work; never re-probe on render.

---

### [UX-12] Aggregate queue ETA on the dashboard

**Category:** UX
**Size:** M
**Touches:** backend, frontend

**Problem or gap:**

Per-job ETA exists, but a user who just enqueued 800 files has no idea whether the whole queue finishes tonight or next week.

**Idea:**

Compute a rolling queue-wide ETA: take recent completed-job throughput (bytes/sec or seconds-of-video/sec by codec), multiply by remaining queued work, divide by the concurrency limit. Show "~Library finishes: Thu 02:00" on the dashboard, with a confidence band.

**First step:**

Add a backend endpoint that returns `{ remaining_jobs, est_seconds_remaining, sample_size }` from recent `encode_stats`, and render it as plain text before adding any band/visualization.

**Risks / tradeoffs:**

Throughput varies wildly by content; present a range, not a false-precision timestamp, and degrade gracefully when the sample is small.

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

### [INT-5] Home Assistant Integration

**Category:** Integration
**Size:** M
**Touches:** backend, docs

**Problem or gap:**

HomeLab enthusiasts use Home Assistant to monitor their infrastructure. Today, there's no way to trigger a "Cinema Mode" lighting scene based on Alchemist finishing an encode, or to see "Current Transcode Speed" on a wall-mounted tablet.

**Idea:**

Implement a native Home Assistant integration (HACS-compatible or core-eligible). Backend provides a long-lived sensor API; the integration maps Alchemist states to HA sensors: `sensor.alchemist_status`, `sensor.alchemist_queue_depth`, `binary_sensor.alchemist_encoding`. Support triggers for "Job Finished" and "Job Failed".

**First step:**

Expand the `GET /api/widget/summary` (from INT-4) to include everything an HA sensor would need, then document a YAML-based `rest` sensor for HA users.
**Risks / tradeoffs:**

None significant — leverage existing API token infrastructure for security.

### [INT-6] Model Context Protocol (MCP) Server

**Category:** Integration
**Size:** S
**Touches:** backend
**Status:** V1 implemented as a read-only stdio MCP server

**Problem or gap:**

Users are increasingly using AI agents (OpenClaw, etc.) to manage their infrastructure. There is currently no standardized way for an AI to "reason" about the Alchemist queue or trigger maintenance tasks.

**Idea:**

Expose an MCP server interface directly from the Alchemist binary. V1 is intentionally read-only: status, job summaries, recent jobs, savings, scan state, and system health.

**First step:**

Done: implement a protocol-shaped `--mcp` stdio server with read-only tools and unit coverage. Future action tools should require a separate safety design.

**Risks / tradeoffs:**

None.

---

### [INT-7] Jellyfin Plugin (C#)

**Category:** Integration
**Size:** L
**Touches:** external (C#)
**Status:** V1 plugin implemented; live Jellyfin validation and release packaging remain

**Problem or gap:**

While Alchemist can trigger library refreshes, a deeper integration would allow Jellyfin users to see transcode status or trigger "Optimize this file" directly from the Jellyfin UI.

**Idea:**

Develop a C#-based Jellyfin plugin that communicates with the Alchemist API. V1 listens for Jellyfin library item additions/updates, forwards eligible local paths to Alchemist with dry-run and path translation controls, listens for completed-job events, and refreshes the containing Jellyfin directory.

**First step:**

Done: create a compileable Jellyfin plugin with configuration UI, connection/event tests, library hook service, path filtering, forward/reverse path translation, enqueue client, completed-job SSE listener, and containing-directory refresh.

**Risks / tradeoffs:**

Requires C# development and maintenance of a separate codebase.

---

### [INT-8] Scoped generic import webhook

**Category:** Integration
**Size:** S
**Touches:** backend, docs

**Problem or gap:**

ARR webhooks cover Sonarr and Radarr, but many self-hosted stacks still import files through qBittorrent, SABnzbd, FileBot, custom scripts, or media managers that only know how to send a generic webhook. Today those tools need a broad `full_access` token or a bespoke script that calls internal queue APIs.

**Idea:**

Add `POST /api/webhooks/import` with a narrow `import_webhook` token scope. The payload accepts a path, optional source label, optional external event ID for dedupe, and optional dry-run flag. It reuses the same submitted-path validation, path translation, dedupe, and enqueue rules as manual enqueue and ARR ingest.

**First step:**

Add the token scope and a handler that accepts `{ "path": "..." }`, performs validation, and returns the same structured enqueue response as the ARR path.

**Risks / tradeoffs:**

Generic webhooks can become an escape hatch for unsafe paths; keep the scope narrow and reuse the existing library-root restrictions.

---

### [INT-9] Radarr / Sonarr tag synchronization

**Category:** Integrations
**Size:** M
**Touches:** backend, config

**Problem or gap:**

INT-1 ingests ARR webhooks, but the relationship is one-way. After Alchemist replaces a file with an AV1 encode, the ARRs still believe the original release is on disk — a quality-profile upgrade or a manual "search" can re-download and overwrite Alchemist's work.

**Idea:**

After a successful replace, call the Radarr/Sonarr API to add a tag (e.g. `alchemist-av1`) to the movie/episode, and optionally honor an inbound `alchemist-skip` tag so users can exempt titles. The tag also gives users a filter in the ARRs to see what's been processed.

**First step:**

Add config for ARR base URL + API key (reuse the ARR settings surface) and implement the tag-add call behind a feature flag; test against a live Radarr instance with one title.

**Risks / tradeoffs:**

Couples Alchemist to ARR API versions; keep it best-effort — a failed tag call must not fail the job, just log.

---

## Performance

### [PERF-1] FFprobe result cache
**Status:** Implemented in 0.3.2-rc.1. `media_probe_cache` table keyed by path/mtime/size/ffprobe-version, read/write in `src/db/probe_cache.rs`, used by `FfmpegAnalyzer::analyze_with_cache()` in `src/media/analyzer.rs`. Schema migration `20260424160000_media_probe_cache.sql`.

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

### [PERF-4] Idle pre-analysis queue

**Category:** Performance
**Size:** M
**Touches:** backend, scheduler, config

**Problem or gap:**

Large libraries still make the first useful scan feel expensive because probing and planning happen close to the moment a user wants work queued. The ffprobe cache helps repeated scans, but Alchemist could do more useful preparation while the engine is idle or inside an off-peak window.

**Idea:**

Add an optional pre-analysis queue that walks changed watch folders, refreshes probe cache entries, and records planner-ready summaries without enqueueing encode work. The feature should obey the scheduler, pause when active encodes need resources, and expose progress through system status. Users get faster "real" scans later because the expensive metadata work is already warm.

**First step:**

Create a disabled-by-default config flag and a CLI-only pre-analysis command that refreshes probe cache entries for one folder without enqueueing jobs.

**Risks / tradeoffs:**

Background work can surprise users on slow disks; keep it opt-in and clearly bounded by concurrency and schedule settings.

---

### [PERF-5] Background probe-cache pre-warming

**Category:** Performance
**Size:** S
**Touches:** backend, system

**Problem or gap:**

When a user adds a watch folder, the first interaction with it — a library preview (F-2), an Intelligence run, or the first scan — pays the full cold ffprobe cost serially. The UI feels slow precisely at the moment of first impression.

**Idea:**

When a watch dir is added, spawn a low-priority bounded task pool that walks the tree and runs *ffprobe only* (no encode planning) to hydrate `media_probe_cache`. By the time the user opens a preview or the planner runs, most files are warm. Gated by a concurrency cap and the off-peak scheduler.

**First step:**

Hook watch-dir creation to enqueue a pre-warm task that probes up to N files with `analyze_with_cache`, and confirm cache rows appear without blocking the request.

**Risks / tradeoffs:**

Spawning probes on disk-add can surprise users on slow NAS volumes; make it opt-in, respect the engine pause state, and cap concurrency low.

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

### [POL-4] Refined About Screen

**Category:** Polish
**Size:** S
**Touches:** frontend

**Problem or gap:**

The current About screen is functional but lacks visual flair and specific system insights. It doesn't feel integrated with the "premium" feel of the rest of the app.

**Idea:**

Improve the About modal with smoother animations (e.g., morphing transitions similar to the system modal). Surface more specific information like the exact build hash, detected hardware backend versions, and a credits section for contributors.

**First step:**

Update `AboutDialog.tsx` with Framer Motion animations for the opening/closing transition.

**Risks / tradeoffs:**

None.

---

### [POL-5] Official Branding & Logo

**Category:** Polish
**Size:** S
**Touches:** frontend, docs

**Problem or gap:**

Alchemist currently lacks a unique visual identity or logo. It uses generic icons in the UI and README.

**Idea:**

Establish an official logo: a stylized Delta (∆) where the top-right side is a Helios orange arrow. Use it consistently across approved WebUI, documentation, and social preview surfaces.

**First step:**

Draft the SVG logo and add only the approved brand assets.

**Risks / tradeoffs:**

None.

---

### [POL-6] Absolute-time hover for relative timestamps

**Category:** Polish
**Size:** S
**Touches:** frontend

**Problem or gap:**

Jobs, scans, tokens, and settings screens often show human-friendly relative times, but troubleshooting usually needs the exact timestamp. A user comparing Alchemist logs with Jellyfin, Sonarr, or systemd has to infer the real time manually.

**Idea:**

Standardize timestamp rendering so compact views show relative time while hover/focus exposes the exact local timestamp and UTC value. Use one shared React helper for job rows, job detail, API token creation, scan status, and notification history surfaces.

**First step:**

Add a small `TimeDisplay` component and replace one timestamp in `JobsTable.tsx` plus one in `JobDetailModal.tsx`.

**Risks / tradeoffs:**

Too much timestamp detail can clutter dense tables; keep exact values in tooltips or secondary text.

---

### [POL-7] Human-readable FFmpeg error summaries

**Category:** Polish
**Size:** M
**Touches:** backend, frontend

**Problem or gap:**

When an encode fails, the job detail surfaces a reason code plus raw FFmpeg stderr. A user sees a wall of hex and codec jargon and can't tell "my GPU ran out of memory" from "the source file is corrupt".

**Idea:**

Extend the existing failure-explanation classifier with a heuristic pass over FFmpeg stderr that recognizes common signatures — NVENC out-of-memory, unsupported pixel format, corrupt input packet, no space left on device — and emits a clean badge + one-sentence remedy. The raw log stays available behind a disclosure.

**First step:**

Collect a handful of real failing stderr samples, add a `classify_ffmpeg_stderr(&str) -> Option<KnownFailure>` function with table-driven matches and unit tests, and surface the result in the existing failure explanation payload.

**Risks / tradeoffs:**

Heuristics drift as FFmpeg versions change; keep the match table small, well-tested, and always fall back to the raw log.

---

### [POL-8] Live FFmpeg filter-graph visualization

**Category:** Polish
**Size:** L
**Touches:** backend, frontend

**Problem or gap:**

`FFmpegCommandBuilder` assembles long argument chains (hwaccel, filters, encoder flags). When an encode behaves unexpectedly, neither the user nor a maintainer has a readable view of the actual pipeline that ran.

**Idea:**

Render the planned pipeline for a job as a small node graph — input → decoder → filters (crop/scale/tonemap) → encoder → muxer → output — derived from the same structured plan the builder consumes. Static (planned) first; a live variant that highlights the active stage is a follow-up.

**First step:**

Have `FFmpegCommandBuilder` expose a structured `PipelineDescription` (it already owns the pieces) and render it as an indented text outline in the job detail before attempting a real graph.

**Risks / tradeoffs:**

A graph that drifts from the real args is worse than none; derive it from the same plan struct the builder uses, never from re-parsing the arg vector.

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

### [OBS-4] Library Health Dashboard

**Category:** Observability
**Size:** M
**Touches:** backend, frontend, schema

**Problem or gap:**

"Library Doctor" exists but it's a one-shot scan. There's no persistent dashboard showing the overall "health" of the library (e.g., % of files with valid streams, count of corrupt headers, files missing audio tracks) over time.

**Idea:**

Turn Library Doctor results into persistent DB rows. Add a "Health" tab in Stats that visualizes: 1) Historical health trend (is the library getting cleaner?), 2) Breakdown by error type (Corrupt, Missing Subtitles, Slow HDD), 3) A "Wall of Shame" for the most problematic files.

**First step:**

Add a `library_health_results` table that persists the output of the current scanner, rather than just returning it to the UI and discarding it.
**Risks / tradeoffs:**

Full library health scans are heavy; needs to be scheduled or throttled to avoid impacting active encodes.

---

### [OBS-5] Redacted diagnostics bundle

**Category:** Observability
**Size:** M
**Touches:** backend, frontend, docs

**Problem or gap:**

When an install misbehaves, the useful evidence is scattered across logs, config, hardware probe output, FFmpeg versions, job failures, and release metadata. Asking users to paste those pieces manually is slow and risks leaking API tokens, paths, webhook URLs, or other private details.

**Idea:**

Add a "Download diagnostics bundle" action in Runtime settings and a matching CLI command. The bundle should include build/version info, OS/arch, FFmpeg/FFprobe versions, redacted config, hardware probe log, recent structured job failures, recent server logs, and current queue counts. Redaction should happen server-side with an allowlist of safe fields.

**First step:**

Implement a CLI-only `alchemist diagnostics --output diagnostics.zip` command that emits version, platform, FFmpeg versions, and a redacted config snapshot.

**Risks / tradeoffs:**

Redaction mistakes are privacy bugs; use allowlist output and tests for token/path/url masking before adding the UI.

---

### [OBS-6] Per-encode energy and cost estimate

**Category:** Observability
**Size:** M
**Touches:** backend, frontend, config

**Problem or gap:**

Encoding is the single biggest power draw a homelab user adds by running Alchemist, but the app gives no sense of what a queue costs to run. Off-peak scheduling exists partly for cost reasons, yet the cost itself is invisible.

**Idea:**

Estimate energy per job from encode wall-time × an average-power figure (user-supplied per-encoder watt estimate, or read from `nvidia-smi` power draw where available). Multiply by a configurable electricity rate to show "this encode ≈ 0.18 kWh ≈ $0.04" in job detail and a running total on the dashboard.

**First step:**

Add config for `electricity_rate` and a per-encoder watt estimate; compute and store `estimated_kwh` on `encode_stats` (additive column) from existing wall-time. Display the per-job number first.

**Risks / tradeoffs:**

Estimates are coarse without real power telemetry; label clearly as estimates and let users tune the watt figure.

---

## Operator

### [OP-4] Self-Installation Flag

**Category:** Operator
**Size:** S
**Touches:** backend, docs

**Problem or gap:**

Binary users currently run Alchemist from their current directory. There is no built-in way to "install" it to a standard system path (like `/opt/alchemist`) or add it to the system `PATH`.

**Idea:**

Add a `--install` or `--install-directory=PATH` flag. When run, Alchemist copies itself to the target directory, sets up necessary permissions, and optionally creates a symbolic link in `/usr/local/bin` (or registers itself in the Windows PATH).

**First step:**

Implement the logic to copy the current executable to a user-specified path.

**Risks / tradeoffs:**

OS-specific permission issues (UAC on Windows, sudo on Linux).

---

### [OP-5] Guided Custom FFmpeg Compilation

**Category:** Operator
**Size:** M
**Touches:** backend, docs

**Problem or gap:**

Pre-built FFmpeg binaries are often "generic". For maximum performance, advanced users may want to compile a version of FFmpeg optimized for their specific CPU/GPU architecture, but the process is daunting.

**Idea:**

Include scripts or a guided CLI tool that assists the user in compiling a customized FFmpeg binary from source. The result would be stored in `.alchemist/ffmpeg` and used preferentially by the pipeline.

**First step:**

Create a documentation page (`docs/ffmpeg-optimization.md`) with a validated "golden" build script for common architectures.

**Risks / tradeoffs:**

Build environments vary wildly; very high support burden if automated.

---
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

### [OP-6] Config validate and diff before apply

**Category:** Operator
**Size:** S
**Touches:** backend, frontend, config

**Problem or gap:**

The config editor is powerful, but operators do not get a clear dry-run answer before applying a broad config change. A typo in a manually edited TOML blob should be caught early, and a legitimate change should explain what will differ after save.

**Idea:**

Add a validation endpoint that accepts candidate config text and returns parse status, normalized config warnings, and a structured diff against the live config. The UI can show additions, changed fields, and high-risk changes such as output paths, replace strategy, token-related settings, and hardware mode. Applying remains a separate explicit action.

**First step:**

Implement `POST /api/settings/config/validate` for parse-and-normalize only, with no persistence. Add a small UI preview in `ConfigEditorSettings.tsx`.

**Risks / tradeoffs:**

Config diffs can expose secrets in the browser; redact sensitive values in both the API response and UI.

---

### [OP-7] Config-change audit log

**Category:** Operator
**Size:** M
**Touches:** backend, frontend, schema

**Problem or gap:**

Settings, profiles, watch dirs, and engine mode can all change at runtime, but there's no record of *what* changed, *when*, or *via which surface* (UI, API token, wizard). When behavior shifts unexpectedly, a self-hoster has no trail to consult.

**Idea:**

Append a row to a `config_audit` table on every accepted config mutation: timestamp, actor (session user or API token name), the setting key, and old→new values (secrets redacted). Surface a read-only "Change history" view in Settings.

**First step:**

Add the additive `config_audit` table and write one row from the settings-bundle update handler; no UI yet — confirm rows land with correct actor attribution.

**Risks / tradeoffs:**

Must redact secret values before persistence, not just before display; an audit log that stores plaintext tokens is a worse leak than no log.

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

### [ENC-4] Film grain preservation mode

**Category:** Encoding
**Size:** M
**Touches:** backend, frontend, config, docs

**Problem or gap:**

AV1 and HEVC encodes can make grain-heavy films look overly smooth even when byte savings look good. Home theater users often care more about preserving texture than maximizing compression, but Alchemist only exposes broad quality profiles today.

**Idea:**

Add an optional `grain_handling` profile setting with conservative choices such as `auto`, `preserve`, and `smooth`. CPU AV1 and x265 can map this to encoder-supported grain/tune behavior; hardware encoders can raise bitrate floors or show a clear "not supported by this backend" explanation. Job detail should record whether grain handling affected the command.

**First step:**

Prototype CPU-only support in the FFmpeg command builder and unit-test the generated SVT-AV1/x265 arguments before exposing any UI.

**Risks / tradeoffs:**

Grain preservation usually costs storage, so the UI must explain that this trades space savings for visual fidelity.

---

### [ENC-5] Sample-based VMAF pre-flight quality gate

**Category:** Encoding
**Size:** L
**Touches:** backend, pipeline, ffmpeg

**Problem or gap:**

Alchemist measures VMAF *after* a full encode. By then the GPU hours are spent — if the result is poor, the user has already paid for it. There's no way to bail early on a title the chosen profile handles badly.

**Idea:**

Before committing to the full encode, encode a short representative slice (e.g. 3×10s segments), measure VMAF on just those, and gate: if the sample VMAF is below a per-profile threshold, either abort with an explanatory failure or auto-bump quality one step and retry the sample. Distinct from ENC-3 (bake-off compares profiles) — this is a go/no-go on the chosen one.

**First step:**

Add a `sample_vmaf(path, plan) -> f64` helper that encodes and scores a few slices; log the predicted score against the eventual full-encode score on real jobs to validate the correlation before gating anything.

**Risks / tradeoffs:**

Sample VMAF can mispredict for unevenly complex titles; pick segments across the runtime and keep the gate opt-in with a conservative threshold.

---

### [ENC-6] Interactive side-by-side encode preview

**Category:** Encoding
**Size:** M
**Touches:** backend, frontend

**Problem or gap:**

A user tuning a custom profile can't see what their CRF/codec choice actually looks like without encoding a whole file and watching it in a separate player.

**Idea:**

Add a "Preview this profile" action that encodes a single ~30s slice with the current settings and serves it next to the same slice from the source for visual A/B comparison in the browser. Reuses the clip-encode path the orchestrator already supports (`clip_start_seconds`/`clip_duration_seconds`).

**First step:**

Wire a `POST /api/profiles/preview-clip` endpoint that produces a short clip to a temp path and streams it back; render a basic two-`<video>` comparison in the profile editor.

**Risks / tradeoffs:**

Browser video tags can't show every codec; fall back to a frame-grab image comparison when the encoded codec isn't browser-playable, and clean up temp clips with an RAII guard.

---

### [ENC-7] Zero-copy hardware decode → encode pipeline

**Category:** Encoding
**Size:** M
**Touches:** backend, ffmpeg

**Problem or gap:**

When both decode and encode run on the same GPU, frames can stay in GPU memory the whole way. If the FFmpeg command doesn't explicitly request it, frames round-trip through system RAM — wasted bandwidth and a CPU bottleneck on low-power NAS hardware.

**Idea:**

In `FFmpegCommandBuilder`, when the decode and encode backends are the same vendor (NVDEC↔NVENC, QSV↔QSV, VAAPI↔VAAPI), emit the flags that keep frames on-device (`-hwaccel_output_format`, matching `-hwaccel`, hardware filter chains). Detect capability once and cache it like `EncoderCapabilities`.

**First step:**

Add a unit test asserting `build_args()` for a same-vendor NVENC job includes `-hwaccel cuda -hwaccel_output_format cuda`, then validate real throughput on hardware.

**Risks / tradeoffs:**

Hardware filter chains differ per vendor and break some filters (crop/tonemap); gate per-platform and fall back to the copy-through path when a filter needs system memory.

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

### [AUTO-3] Disk-space guardrails

**Category:** Automation
**Size:** S
**Touches:** backend, config, frontend

**Problem or gap:**

Long queues can produce large temporary and final outputs. If the temp or output filesystem fills up mid-run, users get avoidable failures and may have to clean partial files manually.

**Idea:**

Add configurable free-space guardrails for temp and output roots. Before starting a job, Alchemist checks available bytes and pauses or skips with a structured reason if the threshold is below the configured minimum. Surface the blocked reason in System Status and send an optional critical notification.

**First step:**

Add a backend guard before job execution that logs and blocks when the output root has less than a fixed 10 GiB free-space threshold. Make it configurable after the behavior is proven.

**Risks / tradeoffs:**

Free-space APIs vary by platform and mount type; fallback behavior must fail safe without blocking forever on unsupported filesystems.

---

### [AUTO-4] Worker-node auto-sleep and Wake-on-LAN

**Category:** Automation
**Size:** M
**Touches:** backend, config

**Problem or gap:**

A homelabber with a dedicated GPU box doesn't want it drawing power 24/7 to service an occasionally-empty queue, but also doesn't want to wake it by hand every time work appears.

**Idea:**

When the queue depth crosses a threshold, issue a Wake-on-LAN magic packet to configured worker MAC addresses; when the queue has been empty for a configurable idle period, issue a sleep/suspend command (SSH or an agent endpoint) to those nodes. Pairs naturally with F-14 swarm mode but is also useful for a single remote node.

**First step:**

Add config for worker MAC/host entries and implement the WoL packet send + a manual "wake now" button; defer the auto-sleep half until F-14 lands.

**Risks / tradeoffs:**

Sleeping a node mid-encode would lose work; only suspend after confirming the node reports zero in-flight jobs, and make the whole feature opt-in.

---

### [AUTO-5] Scheduled recurring library rescan

**Category:** Automation
**Size:** S
**Touches:** backend, config, scheduler

**Problem or gap:**

The file watcher catches new files in real time, but watchers miss events on network mounts, during downtime, or when files are added while Alchemist is stopped. The only catch-up is a manual "Scan Now". New media can sit unprocessed indefinitely.

**Idea:**

Add a configurable recurring full scan (e.g. "every night at 04:00" or "every 6 hours") driven by the existing scheduler. With incremental scan (PERF-3) already landed, a recurring scan is cheap — it mostly prunes unchanged subtrees.

**First step:**

Add a `scan_interval` / `scan_cron` config field and have the scheduler trigger `library_scanner.start_scan()` on that cadence; surface the next-scan time in the UI.

**Risks / tradeoffs:**

A recurring scan during peak hours competes with encodes for disk I/O; default to off-peak and let it respect the engine pause state.

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

### [IMPR-2] Switch to ffmpeg-next crate for better control over ffmpeg

**Category** Improvement
**Size** Potentially large
**Touchs** Backend

**Problem or gap** None

**Idea:**

Transition to then ffmpeg-next crate for better control of ffmpeg?

**First step**

Read the ffmpeg-next documentation and see how 

**Risks / Tradeoffs**

Uncertainty and could be a breaking change


**Idea** random idea, [IMPR-3]

Switch to Mimalloc