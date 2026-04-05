# Alchemist Backlog

Future improvements and features to consider for the project.

---

## Out of Scope — Explicitly Not Planned

These are deliberate design decisions, not omissions. Do not add them.

- **Custom FFmpeg flags / raw flag injection** — Alchemist is designed to be approachable and safe. Exposing raw FFmpeg arguments (whether per-profile, per-job, or in the conversion sandbox) would make it a footgun and undermine the beginner-first design. The encoding pipeline is the abstraction; users configure outcomes, not commands.
- **Distributed encoding across multiple machines** — Not a goal. Alchemist is a single-host tool. Multi-node orchestration is a different product.

---

## High Priority

### Behavior-Preserving Refactor Pass
- Keep the current product behavior exactly the same while improving internal structure
- Refactor `web/src/components/JobManager.tsx` into smaller components and hooks without changing screens, filters, polling, SSE updates, or job actions
- Centralize duplicated byte/time/reduction formatting logic into shared utilities and preserve current output formatting
- Preserve the current realtime model, but make ownership clearer: job/config/system events via SSE, resource metrics via polling
- Add regression coverage around planner decisions, watcher behavior, job lifecycle transitions, and decision explanation rendering before deeper refactors
- Document the current planner heuristics and hardware fallback rules so future cleanup does not accidentally change behavior

### Planning / Simulation Mode
- Add a first-class simulation flow that answers what Alchemist would transcode, remux, or skip without mutating the library
- Show estimated total bytes recoverable, action counts, top skip reasons, and per-file predicted actions
- Support comparing current settings against alternative profiles, codec targets, or threshold snapshots
- Reuse the scanner, analyzer, and planner, but stop before executor and promotion stages

### Per-File Encode History
- When a file has been processed more than once (retry, re-queue after settings change, manual re-run), show the full history of attempts in the job detail panel
- Each attempt should show: date, outcome (completed/failed/skipped), encode stats if applicable (size before/after, codec, duration), and failure reason if failed
- The data is already in the DB across `jobs`, `encode_stats`, and `job_failure_explanations` — this is primarily a UI feature
- Useful for understanding why a file kept failing, or comparing quality before/after a settings change

### E2E Test Coverage
- Expand Playwright tests for more UI flows
- Test job queue management scenarios
- Test error states and recovery flows

### AMD AV1 Validation
- Validate and tune the existing AMD AV1 paths on real hardware
- Cover Linux VAAPI and Windows AMF separately
- Verify encoder selection, fallback behavior, and quality/performance defaults
- Do not treat this as support-from-scratch: encoder wiring and hardware detection already exist

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

### Library Intelligence
- Expand recommendations beyond duplicate detection into remux-only opportunities, wasteful audio layouts, commentary/descriptive-track cleanup, and duplicate-ish title variants
- Keep the feature focused on storage and library quality, not general media management

### Auto-Priority Rules
- Define rules that automatically assign queue priority based on file attributes
- Rule conditions: file path pattern (glob), file age, file size, source watch folder
- Example: "anything under `/movies/` gets priority 2", "files over 20 GB get priority 1"
- Rules evaluated at enqueue time; manual priority overrides still win
- Configured in Settings alongside other library behavior

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

### UI Improvements
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

---

## Low Priority

### API Token Authentication + API Documentation
- Add support for static bearer tokens as an alternative to session cookies
- Enables programmatic access from scripts, home automation (Home Assistant, n8n), and CLI tools without managing session state
- Tokens generated and revoked from Settings; no expiry by default, revocable any time
- Expand API documentation to cover all endpoints with request/response examples

### Passthrough Mode
- A toggle that keeps all watch folders and watcher active but prevents the planner from queuing new jobs
- Different from Pause — Pause stops active encodes; Passthrough lets the system observe and index the library without touching anything
- Useful when testing settings or onboarding a new library without triggering encodes immediately

### Base URL / Subpath Configuration
- Allow Alchemist to be served at a non-root path (e.g. `/alchemist/`) via `ALCHEMIST_BASE_URL`
- Common self-hosting pattern for reverse proxy setups running multiple services on one domain
- Low urgency — most users run Alchemist on a dedicated subdomain or port

### Features from DESIGN_PHILOSOPHY.md
- Add batch job templates

### Code Quality
- Increase test coverage for edge cases
- Add property-based testing for codec parameter generation
- Add fuzzing for FFprobe output parsing

### Documentation
- Add architecture diagrams
- Add contributor guide with development setup
- Video tutorials for common workflows

### Distribution
- Add Homebrew formula
- Add AUR package
- Add Flatpak/Snap packages
- Improve Windows installer (WiX) with auto-updates

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
