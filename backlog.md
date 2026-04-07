# Alchemist Backlog

Future improvements and features to consider for the project.

---

## Out of Scope — Explicitly Not Planned

These are deliberate design decisions, not omissions. Do not add them.

- **Custom FFmpeg flags / raw flag injection** — Alchemist is designed to be approachable and safe. Exposing raw FFmpeg arguments (whether per-profile, per-job, or in the conversion sandbox) would make it a footgun and undermine the beginner-first design. The encoding pipeline is the abstraction; users configure outcomes, not commands.
- **Distributed encoding across multiple machines** — Not a goal. Alchemist is a single-host tool. Multi-node orchestration is a different product.

---

## High Priority

Testing policy for this section:

- Backend/unit/integration coverage and Playwright coverage are exit criteria for each item below.
- Do not treat "more tests" as a standalone product track; attach the required coverage to the feature or refactor that needs it.

### 1. Engine Lifecycle Controls

#### Goal
- Make engine lifecycle controls real, explicit, and operator-safe from the header/About surface.

#### Scope
- Redesign the About screen so it fits the current visual language.
- Add a **Restart Engine** action that restarts the engine loop without killing the Alchemist process.
- Add a **Shutdown Alchemist** action that cancels active jobs immediately and exits the process cleanly.
- Define and surface the lifecycle states needed to make restart and shutdown understandable in the UI.

#### Non-Goals
- Do not re-exec the whole app process to implement restart.
- Do not drain active jobs to completion on shutdown; shutdown means cancel and exit.

#### Dependencies
- Backend lifecycle endpoints and orchestration semantics for restart and shutdown.
- Reliable event/state propagation so the UI can reflect transient lifecycle states without stale polling or SSE behavior.

#### Acceptance Criteria
- Restart tears down and reinitializes the engine loop while the binary stays alive.
- Shutdown stops accepting new work, cancels active jobs, persists the right terminal states, and exits cleanly.
- Job rows, logs, and toasts clearly distinguish pause, drain, restart, cancellation, and shutdown.
- The About surface exposes restart and shutdown with confirmation and clear failure handling.

#### Required Tests
- Backend tests for restart/shutdown semantics and lifecycle state transitions.
- Playwright coverage for About screen controls, confirmations, success states, and failure states.

#### Solution
- Add a dedicated engine lifecycle API instead of overloading pause/drain:
  - Add authenticated lifecycle routes for `restart engine` and `shutdown app`.
  - Keep restart scoped to the engine loop only; do not re-exec the binary.
  - Keep shutdown as cancel-all-and-exit; do not reuse drain semantics.
- Introduce a server-owned shutdown trigger so HTTP-initiated shutdown uses the same shutdown path as Ctrl+C and SIGTERM:
  - Extend `RunServerArgs` and `AppState` with a shutdown signal sender.
  - Update `axum::serve(...).with_graceful_shutdown(...)` to also listen for an internal shutdown signal.
- Add an explicit lifecycle transition guard:
  - Reject overlapping restart/shutdown requests while a lifecycle action is already in progress.
  - Surface lifecycle state through `/api/engine/status` so the UI can render restarting/shutting-down states cleanly.
- Implement restart as an engine-loop reset, not a process restart:
  - Pause new intake.
  - Cancel active jobs immediately through the orchestrator.
  - Clear drain state and any temporary lifecycle flags.
  - Reinitialize the engine loop state needed to resume normal processing.
  - Resume only if the scheduler is not actively pausing the engine.
- Implement shutdown as a process-level cancel-and-exit flow:
  - Pause intake.
  - Cancel all active jobs immediately.
  - Give cancellation and persistence a short bounded window to flush terminal state.
  - Trigger the internal shutdown signal so the server exits through the same top-level path already used for signals.
- Split the backend work by file responsibility:
  - `src/media/processor.rs`: add restart/shutdown lifecycle methods and transient lifecycle state.
  - `src/server/mod.rs`: wire new lifecycle routes and internal shutdown signaling into `AppState` and server startup.
  - `src/server/jobs.rs` or a new dedicated engine/server lifecycle module: implement authenticated handlers for restart/shutdown.
  - `src/main.rs`: keep the top-level exit behavior but make sure HTTP-triggered shutdown lands in the same path as signal-triggered shutdown.
- Update the UI in two passes:
  - Redesign `web/src/components/AboutDialog.tsx` to match the current visual system and include restart/shutdown actions plus confirmation UX.
  - Update `web/src/components/HeaderActions.tsx` and any engine-status consumers to understand the new lifecycle states.
- Add coverage before shipping:
  - Backend tests for restart, shutdown, overlapping request rejection, and status payload transitions.
  - Playwright tests for About modal actions, confirmation dialogs, success flows, disabled/loading states, and failure toasts.

### 2. Planner and Lifecycle Documentation

#### Goal
- Lock down current behavior before deeper refactors by documenting planner heuristics, hardware fallback rules, and engine lifecycle semantics.

#### Scope
- Document the current planner heuristics and stable skip/transcode/remux decision boundaries.
- Document hardware fallback rules and vendor/backend selection semantics.
- Document lifecycle semantics for pause, drain, restart, cancel, and shutdown.

#### Non-Goals
- No product behavior changes.
- No speculative redesign of the planner or lifecycle model.

#### Dependencies
- Cross-check against the existing backend behavior and tests, not just intended behavior.

#### Acceptance Criteria
- Future cleanup work has a single documented source of truth for planner and lifecycle behavior.
- The docs are specific enough to catch accidental behavior changes during refactors.

#### Required Tests
- Add or tighten assertions where documentation work uncovers missing coverage around planner decisions, hardware fallback, or lifecycle states.

#### Solution

### 3. Per-File Encode History

#### Goal
- Show a complete attempt history in the job detail panel for files that have been processed more than once.

#### Scope
- Group history by canonical file identity rather than path-only matching.
- Show date, outcome, encode stats where applicable, and failure reason where applicable.
- Make repeated retries, re-queues after settings changes, and manual reruns understandable at a glance.

#### Non-Goals
- Do not turn this into a general media-management timeline.
- Do not rely on path-only grouping when a canonical identity is available.

#### Dependencies
- Query shaping across `jobs`, `encode_stats`, and `job_failure_explanations`.
- A stable canonical file identity strategy that survives path changes better than naive path matching.

#### Acceptance Criteria
- Job detail shows prior attempts for the same canonical file identity with enough detail to explain repeated outcomes.
- Operators can distinguish retry noise from truly separate processing attempts.

#### Required Tests
- Backend coverage for history lookup and canonical identity grouping.
- UI coverage for rendering mixed completed/failed/skipped histories.

#### Solution

### 4. Behavior-Preserving Refactor Pass

#### Goal
- Improve internal structure without changing visible product behavior.

#### Scope
- Refactor `web/src/components/JobManager.tsx` into smaller components and hooks without changing screens, filters, polling, SSE updates, or job actions.
- Centralize duplicated byte/time/reduction formatting logic into shared utilities while preserving current output formatting.
- Preserve the current realtime model, but make ownership clearer: job/config/system events via SSE, resource metrics via polling.
- Add regression coverage around planner decisions, watcher behavior, job lifecycle transitions, and decision explanation rendering before deeper refactors.

#### Non-Goals
- No new screens, filters, realtime behaviors, or job actions.
- No opportunistic product changes hidden inside the refactor.

#### Dependencies
- Planner/lifecycle documentation and regression coverage should land before deeper structural work.

#### Acceptance Criteria
- Existing behavior, strings, filters, and action flows remain stable.
- `JobManager` is decomposed enough that future feature work does not require editing a single monolithic file for unrelated changes.
- Realtime ownership is easier to reason about and less likely to regress.

#### Required Tests
- Keep current backend and Playwright suites green.
- Add targeted regression coverage before extracting behavior into hooks/components.

#### Solution

### 5. AMD AV1 Validation

#### Goal
- Validate and tune the existing AMD AV1 paths on real hardware.

#### Scope
- Cover Linux VAAPI and Windows AMF separately.
- Verify encoder selection, fallback behavior, and quality/performance defaults.
- Treat this as validation/tuning of existing wiring, not support-from-scratch.

#### Non-Goals
- Do not expand the stable support promise before validation is complete.
- Do not invent a fake validation story without real hardware runs.

#### Dependencies
- Access to representative Linux VAAPI and Windows AMF hardware.
- Repeatable manual verification notes and any scripted checks that can be automated.

#### Acceptance Criteria
- AMD AV1 is either validated with documented defaults and caveats, or explicitly left outside the supported matrix with clearer docs.
- Linux and Windows results are documented separately.

#### Required Tests
- Scripted verification where possible, plus recorded manual validation runs on real hardware.

#### Solution

---

## Medium Priority

### Power User Conversion / Remux Mode
**Target: 0.3.1**

#### Overview
- Introduce a conversion mode that allows users to upload a single file and perform customizable transcoding or remuxing operations using Alchemist's existing pipeline
- Exposes the same encoding parameters Alchemist uses internally — no raw flag injection
- Clear separation between remux mode (container-only, lossless) and transcode mode (re-encode)

#### Goals
- Provide a fast, interactive way to process single files
- Reuse Alchemist's existing job queue and worker system
- Avoid becoming a HandBrake clone; prioritize clarity over exhaustive configurability

#### Storage Structure
- Store temporary files under `~/.alchemist/temp/`

```text
~/.alchemist/
  temp/
    uploads/     # raw uploaded files
    outputs/     # processed outputs
    jobs/        # job metadata (JSON)
```

- Each job gets a unique ID (UUID or short hash)
- Files stored per job:
  `uploads/{job_id}/input.ext`
  `outputs/{job_id}/output.ext`
  `jobs/{job_id}.json`

#### Core Workflow
1. User uploads file (drag-and-drop or file picker)
2. File is stored in `~/.alchemist/temp/uploads/{job_id}/`
3. Media is probed (`ffprobe`) and stream info is displayed
4. User configures conversion settings
5. User submits job
6. Job is added to Alchemist queue
7. Worker processes job using standard pipeline
8. Output is saved to `~/.alchemist/temp/outputs/{job_id}/`
9. User downloads result

#### UI Design Principles
- Must feel like a visual encoding editor
- No oversimplified presets as the primary UX
- All major encoding options exposed
- Clear separation between remux and transcode modes

#### UI Sections
##### 1. Input
- File upload (drag-and-drop)
- Display:
  - container format
  - video streams (codec, resolution, HDR info)
  - audio streams (codec, channels)
  - subtitle streams

##### 2. Output Container
- Options: `mkv`, `mp4`, `webm`, `mov`

##### 3. Video Settings
- Codec: `copy`, `h264`, `hevc`, `av1`
- Mode: CRF (quality-based) or Bitrate (kbps)
- Preset: `ultrafast` to `veryslow`
- Resolution: original, custom (width/height), scale factor
- HDR: preserve, tonemap to SDR, strip metadata

##### 4. Audio Settings
- Codec: `copy`, `aac`, `opus`, `mp3`
- Bitrate
- Channels (`auto`, stereo, 5.1, etc.)

##### 5. Subtitle Settings
- Options: `copy`, burn-in, remove

##### 6. Remux Mode
- Toggle: `[ ] Remux only (no re-encode)`
- Forces stream copy, disables all encoding options
- Use cases: container changes, stream compatibility fixes, zero quality loss operations

##### 7. Command Preview
- Display the generated FFmpeg command before execution
- Example: `ffmpeg -i input.mkv -c:v libaom-av1 -crf 28 -b:v 0 -c:a opus output.mkv`
- Read-only — for transparency and debugging, not for editing

#### Job System Integration
- Use the existing Alchemist job queue
- Treat each conversion as a standard job
- Stream logs live to the UI

#### Job Metadata Example
```json
{
  "id": "abc123",
  "input_path": "...",
  "output_path": "...",
  "mode": "transcode | remux",
  "video": { "codec": "av1", "crf": 28, "preset": "slow" },
  "audio": { "codec": "opus", "bitrate": 128 },
  "container": "mkv",
  "status": "queued"
}
```

#### Cleanup Strategy
- Auto-delete uploads after X hours
- Auto-delete outputs after download or timeout
- Enforce a max file size limit
- Run a periodic cleanup job that scans the temp directory

#### Security Considerations
- Sanitize filenames
- Prevent path traversal
- Validate file types via probing, not extension
- Isolate the temp directory
- Do not allow arbitrary file path input

#### Non-Goals
- Not a beginner-focused tool
- Not a replacement for full automation workflows
- Not a cloud encoding service; no public hosting assumed
- No raw FFmpeg flag injection (see Out of Scope)

#### Solution

### Library Intelligence
- Expand recommendations beyond duplicate detection into remux-only opportunities, wasteful audio layouts, commentary/descriptive-track cleanup, and duplicate-ish title variants
- Keep the feature focused on storage and library quality, not general media management

#### Solution

### Auto-Priority Rules
- Define rules that automatically assign queue priority based on file attributes
- Rule conditions: file path pattern (glob), file age, file size, source watch folder
- Example: "anything under `/movies/` gets priority 2", "files over 20 GB get priority 1"
- Rules evaluated at enqueue time; manual priority overrides still win
- Configured in Settings alongside other library behavior

#### Solution

### Performance Optimizations
- Profile scanner/analyzer hot paths before changing behavior
- Only tune connection pooling after measuring database contention under load
- Consider caching repeated FFprobe calls on identical files if profiling shows probe churn is material

#### Solution

### Audio Normalization
- Apply EBU R128 loudness normalization to audio streams during transcode
- Target: -23 LUFS integrated, -1 dBTP true peak (broadcast standard)
- Opt-in per library profile, disabled by default
- Implemented via `loudnorm` FFmpeg filter — no new dependencies
- Two-pass mode for accurate results; single-pass for speed
- Should surface loudness stats (measured LUFS, correction applied) in
  the job detail panel alongside existing encode stats
- Do not normalize if audio is being copied (copy mode bypasses this)

#### Solution

### UI Improvements
- Add keyboard shortcuts for common actions

#### Solution

### Notification Improvements
- **Granular event types** — current events are too coarse. Add:
    - `encode.started` — job moved from queued to encoding
    - `encode.completed` — with savings summary (size before/after)
    - `encode.failed` — with failure reason included in payload
    - `scan.completed` — N files discovered, M queued
    - `engine.idle` — queue drained, nothing left to process
    - `daily.summary` — opt-in digest of the day's activity
- **Per-target event filtering** — each notification target should
  independently choose which events it receives. Currently, all targets
  get the same events. A Discord webhook might want everything; a
  phone webhook might only want failures.
- **Richer payloads** — completed job notifications should include
  filename, input size, output size, space saved, and encode time.
  Currently, the payload is minimal.
- **Add Telegram integration** — bot token + chat ID, same event
  model as Discord. No new dependencies needed (reqwest already present).
- **Improve Discord notifications** — add bot token support where it meaningfully improves delivery or richer messaging.
- **Add email support** — SMTP with TLS. Lower priority than Telegram.
  Most self-hosters already have Discord or Telegram.

#### Solution

---

## Low Priority

### Planning / Simulation Mode
- Not a current focus. If revisited, start with a single current-config dry-run before attempting comparison mode.
- Add a first-class simulation flow that answers what Alchemist would transcode, remux, or skip without mutating the library.
- Show estimated total bytes recoverable, action counts, top skip reasons, and per-file predicted actions.
- Reuse the scanner, analyzer, and planner, but stop before executor and promotion stages.
- Only add profile/codec/threshold comparison snapshots after the simple single-config flow proves useful.

#### Solution

### API Token Authentication + API Documentation
- Add support for static bearer tokens as an alternative to session cookies
- Enables programmatic access from scripts, home automation (Home Assistant, n8n), and CLI tools without managing session state
- Tokens generated and revoked from Settings; no expiry by default, revocable any time
- Expand API documentation to cover all endpoints with request/response examples

#### Solution

### Passthrough Mode
- A toggle that keeps all watch folders and watcher active but prevents the planner from queuing new jobs
- Different from Pause — Pause stops active encodes; Passthrough lets the system observe and index the library without touching anything
- Useful when testing settings or onboarding a new library without triggering encodes immediately

#### Solution

### Base URL / Subpath Configuration
- Allow Alchemist to be served at a non-root path (e.g. `/alchemist/`) via `ALCHEMIST_BASE_URL`
- Common self-hosting pattern for reverse proxy setups running multiple services on one domain
- Low urgency — most users run Alchemist on a dedicated subdomain or port

#### Solution

### Features from DESIGN_PHILOSOPHY.md
- Add batch job templates

#### Solution

### Code Quality
- Increase test coverage for edge cases
- Add property-based testing for codec parameter generation
- Add fuzzing for FFprobe output parsing

#### Solution

### Documentation
- Add architecture diagrams
- Add contributor guide with development setup
- Video tutorials for common workflows

#### Solution

### Distribution
- Add Homebrew formula
- Add AUR package
- Add Flatpak/Snap packages
- Improve Windows installer (WiX) with auto-updates

#### Solution

---

## Completed (Recent)

- [x] Split server.rs into modules
- [x] Add typed broadcast channels
- [x] Add security headers middleware
- [x] Add database query timeouts
- [x] Add config file permission check
- [x] Handle SSE lagged events in frontend
- [x] Create FFmpeg integration tests
- [x] Expand documentation site
- [x] Pin MSRV in Cargo.toml
- [x] Add schema versioning for migrations
- [x] Enable SQLite WAL mode
- [x] Add theme persistence and selection
- [x] Add job history filtering and search
- [x] Add subtitle extraction sidecars
- [x] Decision clarity — structured skip/failure explanations with codes, plain-English summaries, measured values, and operator guidance
- [x] Retry backoff visibility — countdown on failed jobs, attempt count in job detail
- [x] Per-library profiles (Space Saver, Quality First, Balanced, Streaming)
- [x] Engine runtime modes (Background / Balanced / Throughput) with drain support
- [x] Container remuxing (MP4 → MKV lossless)
- [x] Stream rules (commentary stripping, language filtering, default-only audio)
- [x] VMAF quality gating
- [x] Library Intelligence duplicate detection
- [x] Library Doctor health scanning
- [x] Boot auto-analysis
- [x] Mobile layout
