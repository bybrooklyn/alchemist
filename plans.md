# Open Item Plans

---

## [UX-2] Single File Enqueue

### Goal
`POST /api/jobs/enqueue` + "Add file" button in JobsToolbar.

### Backend

**New handler in `src/server/jobs.rs`:**
```rust
#[derive(Deserialize)]
struct EnqueueFilePayload {
    input_path: String,
    source_root: Option<String>,
}

async fn enqueue_file_handler(State(state), Json(payload)) -> impl IntoResponse
```

Logic:
1. Validate `input_path` exists on disk, is a file
2. Read `mtime` from filesystem metadata
3. Build `DiscoveredMedia { path, mtime, source_root }`
4. Call `enqueue_discovered_with_db(&db, discovered)` — reuses all existing skip checks, output path computation, file settings
5. If `Ok(true)` → fetch job via `db.get_job_by_input_path()`, return it
6. If `Ok(false)` → 409 "already tracked / output exists"
7. If `Err` → 400 with error

**Route:** Add `.route("/api/jobs/enqueue", post(enqueue_file_handler))` in `src/server/mod.rs`

### Frontend

**`web/src/components/jobs/JobsToolbar.tsx`:**
- Add "Add File" button next to refresh
- Opens small modal/dialog with text input for path
- POST to `/api/jobs/enqueue`, toast result
- SSE handles job appearing in table automatically

### Files to modify
- `src/server/jobs.rs` — new handler + payload struct
- `src/server/mod.rs` — route registration
- `web/src/components/jobs/JobsToolbar.tsx` — button + dialog
- `web/src/components/jobs/` — optional: new `EnqueueDialog.tsx` component

### Verification
- `cargo check && cargo test && cargo clippy`
- Manual: POST valid path → job appears queued
- POST nonexistent path → 400
- POST already-tracked path → 409
- Frontend: click Add File, enter path, see job in table

---

## [UX-3] Workers-Blocked Reason

### Goal
Surface why queued jobs aren't being processed. Extend `/api/engine/status` → show reason in JobDetailModal.

### Backend

**Extend `engine_status_handler` response** (or create new endpoint) to include blocking state:

```rust
struct EngineStatusResponse {
    // existing fields...
    blocked_reason: Option<String>,  // "paused", "scheduled", "draining", "boot_analysis", "slots_full", null
    schedule_resume: Option<String>, // next window open time if scheduler_paused
}
```

Derive from `Agent` state:
- `agent.is_manual_paused()` → `"paused"`
- `agent.is_scheduler_paused()` → `"scheduled"`
- `agent.is_draining()` → `"draining"`
- `agent.is_boot_analyzing()` → `"boot_analysis"`
- `agent.in_flight_jobs >= agent.concurrent_jobs_limit()` → `"slots_full"`
- else → `null` (processing normally)

### Frontend

**`web/src/components/jobs/JobDetailModal.tsx`:**
- Below queue position display, show blocked reason if present
- Fetch from engine status (already available via SSE `EngineStatusChanged` events, or poll `/api/engine/status`)
- Color-coded: yellow for schedule/pause, blue for boot analysis, gray for slots full

### Files to modify
- `src/server/jobs.rs` or wherever `engine_status_handler` lives — extend response
- `web/src/components/jobs/JobDetailModal.tsx` — display blocked reason
- `web/src/components/jobs/useJobSSE.ts` — optionally track engine status via SSE

### Verification
- Pause engine → queued job detail shows "Engine paused"
- Set schedule window outside current time → shows "Outside schedule window"
- Fill all slots → shows "All worker slots occupied"
- Resume → reason disappears

---

## [FG-4] Intelligence Page Actions

### Goal
Add actionable buttons to `LibraryIntelligence.tsx`: delete duplicates, queue remux opportunities.

### Duplicate Group Actions

**"Keep Latest, Delete Rest" button per group:**
- Each duplicate group card gets a "Clean Up" button
- Selects all jobs except the one with latest `updated_at`
- Calls `POST /api/jobs/batch` with `{ action: "delete", ids: [...] }`
- Confirmation modal: "Archive N duplicate jobs?"

**"Clean All Duplicates" bulk button:**
- Top-level button in duplicates section header
- Same logic across all groups
- Shows total count in confirmation

### Recommendation Actions

**"Queue All Remux" button:**
- Gathers IDs of all remux opportunity jobs
- Calls `POST /api/jobs/batch` with `{ action: "restart", ids: [...] }`
- Jobs re-enter queue for remux processing

**Per-recommendation "Queue" button:**
- Individual restart for single recommendation items

### Backend
No new endpoints needed — existing `POST /api/jobs/batch` handles all actions (cancel/delete/restart).

### Frontend

**`web/src/components/LibraryIntelligence.tsx`:**
- Add "Clean Up" button to each duplicate group card
- Add "Clean All Duplicates" button to section header
- Add "Queue All" button to remux opportunities section
- Add confirmation modal component
- Add toast notifications for success/error
- Refresh data after action completes

### Files to modify
- `web/src/components/LibraryIntelligence.tsx` — buttons, modals, action handlers

### Verification
- Click "Clean Up" on duplicate group → archives all but latest
- Click "Queue All Remux" → remux jobs reset to queued
- Confirm counts in modal match actual
- Data refreshes after action

---

## [RG-2] AMD VAAPI/AMF Validation

### Goal
Verify AMD hardware encoder paths produce correct FFmpeg commands on real AMD hardware.

### Problem
`src/media/ffmpeg/vaapi.rs` and `src/media/ffmpeg/amf.rs` were implemented without real hardware validation. Flag mappings, device paths, and quality controls may be incorrect.

### Validation checklist

**VAAPI (Linux):**
- [ ] Device path `/dev/dri/renderD128` detection works
- [ ] `hevc_vaapi` / `h264_vaapi` encoder selection
- [ ] CRF/quality mapping → `-rc_mode CQP -qp N` or `-rc_mode ICQ -quality N`
- [ ] HDR passthrough flags (if applicable)
- [ ] Container compatibility (MKV/MP4)

**AMF (Windows):**
- [ ] `hevc_amf` / `h264_amf` encoder selection
- [ ] Quality mapping → `-quality quality -qp_i N -qp_p N`
- [ ] B-frame support detection
- [ ] HDR passthrough

### Approach
1. Write unit tests for `build_args()` output — verify flag strings without hardware
2. Gate integration tests on `AMD_GPU_AVAILABLE` env var
3. Document known-good flag sets from AMD documentation
4. Add `EncoderCapabilities` detection for AMF/VAAPI (similar to existing NVENC/QSV detection)

### Files to modify
- `src/media/ffmpeg/vaapi.rs` — flag corrections if needed
- `src/media/ffmpeg/amf.rs` — flag corrections if needed
- `tests/` — new integration test file gated on hardware

### Verification
- Unit tests pass on CI (no hardware needed)
- Integration tests pass on AMD hardware (manual)
- Generated FFmpeg commands reviewed against AMD documentation
