# Audit Findings

Last updated: 2026-04-12 (second pass)

---

## P1 Issues

---

### [P1-1] Cancel during analysis can be overwritten by the pipeline

**Status: RESOLVED**

**Files:**
- `src/server/jobs.rs:41–63`
- `src/media/pipeline.rs:1178–1221`
- `src/orchestrator.rs:84–90`

**Severity:** P1

**Problem:**

`request_job_cancel()` in `jobs.rs` immediately writes `Cancelled` to the DB for jobs in `Analyzing` or `Resuming` state. The pipeline used to have race windows where it could overwrite this state with `Skipped`, `Encoding`, or `Remuxing` if it reached a checkpoint after the cancel was issued but before it could be processed.

**Fix:**

Implemented `cancel_requested: Arc<tokio::sync::RwLock<HashSet<i64>>>` in `Transcoder` (orchestrator). The `update_job_state` wrapper in `pipeline.rs` now checks this set before any DB write for `Encoding`, `Remuxing`, `Skipped`, and `Completed` states. Terminal states (Completed, Failed, Cancelled, Skipped) also trigger removal from the set.

---

### [P1-2] VideoToolbox quality controls are effectively ignored

**Status: RESOLVED**

**Files:**
- `src/media/planner.rs:630–650`
- `src/media/ffmpeg/videotoolbox.rs:25–54`
- `src/config.rs:85–92`

**Severity:** P1

**Problem:**

The planner used to emit `RateControl::Cq` values that were incorrectly mapped for VideoToolbox, resulting in uncalibrated or inverted quality.

**Fix:**

Fixed the mapping in `videotoolbox.rs` to use `-q:v` (1-100, lower is better) and clamped the input range to 1-51 to match user expectations from x264/x265. Updated `QualityProfile` in `config.rs` to provide sane default values (24, 28, 32) for VideoToolbox quality.

---

## P2 Issues

---

### [P2-1] Convert does not reuse subtitle/container compatibility checks

**Status: RESOLVED**

**Files:**
- `src/conversion.rs:372–380`
- `src/media/planner.rs`

**Severity:** P2

**Problem:**

The conversion path was not validating subtitle/container compatibility, leading to FFmpeg runtime failures instead of early validation errors.

**Fix:**

Integrated `crate::media::planner::subtitle_copy_supported` into `src/conversion.rs:build_subtitle_plan`. The "copy" mode now returns an `AlchemistError::Config` if the combination is unsupported.

---

### [P2-2] Completed job metadata omitted at the API layer

**Status: RESOLVED**

**Files:**
- `src/db.rs:254–263`
- `src/media/pipeline.rs:599`
- `src/server/jobs.rs:343`

**Severity:** P2

**Problem:**

Job details required a live re-probe of the input file to show metadata, which failed if the file was moved or deleted after completion.

**Fix:**

Added `input_metadata_json` column to the `jobs` table (migration `20260412000000_store_job_metadata.sql`). The pipeline now stores the metadata string immediately after analysis. `get_job_detail_handler` reads this stored value, ensuring metadata is always available even if the source file is missing.

---

### [P2-3] LAN-only setup exposed to reverse proxy misconfig

**Status: RESOLVED**

**Files:**
- `src/config.rs` — `SystemConfig.trusted_proxies`
- `src/server/mod.rs` — `AppState.trusted_proxies`, `AppState.setup_token`
- `src/server/middleware.rs` — `is_trusted_peer`, `request_ip`, `auth_middleware`

**Severity:** P2

**Problem:**

The setup wizard gate trusts all private/loopback IPs for header forwarding. When running behind a misconfigured proxy that doesn't set headers, it falls back to the proxy's own IP (e.g. 127.0.0.1), making the setup endpoint accessible to external traffic.

**Fix:**

Added two independent security layers:
1. `trusted_proxies: Vec<String>` to `SystemConfig`. When non-empty, only those exact IPs (plus loopback) are trusted for proxy header forwarding instead of all RFC-1918 ranges. Empty = previous behavior preserved.
2. `ALCHEMIST_SETUP_TOKEN` env var. When set, setup endpoints require `?token=<value>` query param regardless of client IP. Token mode takes precedence over IP-based LAN check.

---

### [P2-4] N+1 DB update in batch cancel

**Status: RESOLVED**

**Files:**
- `src/server/jobs.rs` — `batch_jobs_handler`

**Severity:** P2

**Problem:**

`batch_jobs_handler` for "cancel" action iterates over jobs and calls `request_job_cancel` which performs an individual `update_job_status` query per job. Cancelling a large number of jobs triggers N queries.

**Fix:**

Restructured the "cancel" branch in `batch_jobs_handler`. Orchestrator in-memory operations (add_cancel_request, cancel_job) still run per-job, but all DB status updates are batched into a single `db.batch_cancel_jobs(&ids)` call (which already existed at db.rs). Immediate-resolution jobs (Queued + successfully signalled Analyzing/Resuming) are collected and written in one UPDATE ... WHERE id IN (...) query.

---

### [P2-5] Missing archived filter in health and stats queries

**Status: RESOLVED**

**Files:**
- `src/db.rs` — `get_aggregated_stats`, `get_job_stats`, `get_health_summary`

**Severity:** P2

**Problem:**

`get_health_summary` and `get_aggregated_stats` (total_jobs) do not include `AND archived = 0`. Archived (deleted) jobs are incorrectly included in library health metrics and total job counts.

**Fix:**

Added `AND archived = 0` to all three affected queries: `total_jobs` and `completed_jobs` subqueries in `get_aggregated_stats`, the `GROUP BY status` query in `get_job_stats`, and both subqueries in `get_health_summary`. Updated tests that were asserting the old (incorrect) behavior.

---

### [P2-6] Daily summary notifications bypass SSRF protections

**Status: RESOLVED**

**Files:**
- `src/notifications.rs` — `build_safe_client()`, `send()`, `send_daily_summary_target()`

**Severity:** P2

**Problem:**

`send_daily_summary_target()` used `Client::new()` without any SSRF defences, while `send()` applied DNS timeout, private-IP blocking, no-redirect policy, and request timeout.

**Fix:**

Extracted all client-building logic into `build_safe_client(&self, target)` which applies the full SSRF defence stack. Both `send()` and `send_daily_summary_target()` now use this shared helper.

---

### [P2-7] Silent reprobe failure corrupts saved encode stats

**Status: RESOLVED**

**Files:**
- `src/media/pipeline.rs` — `finalize_job()` duration reprobe

**Severity:** P2

**Problem:**

When a completed encode's metadata has `duration_secs <= 0.0`, the pipeline reprobes the output file to get the actual duration. If reprobe fails, the error was silently swallowed via `.ok()` and duration defaulted to 0.0, poisoning downstream stats.

**Fix:**

Replaced `.ok().and_then().unwrap_or(0.0)` chain with explicit `match` that logs the error via `tracing::warn!` and falls through to 0.0. Existing guards at the stats computation lines already handle `duration <= 0.0` correctly — operators now see *why* stats are zeroed.

---

## Technical Debt

---

### [TD-1] `db.rs` is a 3481-line monolith

**Status: RESOLVED**

**File:** `src/db/` (was `src/db.rs`)

**Severity:** TD

**Problem:**

The database layer had grown to nearly 3500 lines. Every query, migration flag, and state enum was in one file, making navigation and maintenance difficult.

**Fix:**

Split into `src/db/` module with 8 submodules: `mod.rs` (Db struct, init, migrations, hash fns), `types.rs` (all type defs), `events.rs` (event enums + channels), `jobs.rs` (job CRUD/filtering/decisions), `stats.rs` (encode/aggregated/daily stats), `config.rs` (watch dirs/profiles/notifications/schedules/file settings/preferences), `conversion.rs` (ConversionJob CRUD), `system.rs` (auth/sessions/API tokens/logs/health). All tests moved alongside their methods. Public API unchanged — all types re-exported from `db/mod.rs`.

---

### [TD-2] `AlchemistEvent` legacy bridge is dead weight

**Status: RESOLVED**

**Files:**
- `src/db.rs` — enum and From impls removed
- `src/media/pipeline.rs`, `src/media/executor.rs`, `src/media/processor.rs` — legacy `tx` channel removed
- `src/notifications.rs` — migrated to typed `EventChannels` (jobs + system)
- `src/server/mod.rs`, `src/main.rs` — legacy channel removed from AppState/RunServerArgs

**Severity:** TD

**Problem:**

`AlchemistEvent` was a legacy event type duplicated by `JobEvent`, `ConfigEvent`, and `SystemEvent`. All senders were emitting events on both channels.

**Fix:**

Migrated the notification system (the sole consumer) to subscribe to `EventChannels.jobs` and `EventChannels.system` directly. Added `SystemEvent::EngineIdle` variant. Removed `AlchemistEvent` enum, its `From` impls, the legacy `tx` broadcast channel from all structs, and the `pub use` from `lib.rs`.

---

### [TD-3] `pipeline.rs` legacy `AlchemistEvent::Progress` stub

**Status: RESOLVED**

**Files:**
- `src/media/pipeline.rs:1228`

**Severity:** TD

**Problem:**

The pipeline used to emit zeroed progress events that could overwrite real stats from the executor.

**Fix:**

Emission removed. A comment at line 1228-1229 confirms that `AlchemistEvent::Progress` is no longer emitted from the pipeline wrapper.

---

### [TD-4] Silent `.ok()` on pipeline decision and attempt DB writes

**Status: RESOLVED**

**Files:**
- `src/media/pipeline.rs` — all `add_decision`, `insert_encode_attempt`, `upsert_job_failure_explanation`, and `add_log` call sites

**Severity:** TD

**Problem:**

Decision records, encode attempt records, failure explanations, and error logs were written with `.ok()` or `let _ =`, silently discarding DB write failures. These records are the only audit trail of *why* a job was skipped/transcoded/failed.

**Fix:**

Replaced all `.ok()` / `let _ =` patterns on `add_decision`, `insert_encode_attempt`, `upsert_job_failure_explanation`, and `add_log` calls with `if let Err(e) = ... { tracing::warn!(...) }`. Pipeline still continues on failure, but operators now see the error.

---

### [TD-5] Correlated subquery for sort-by-size in job listing

**Status: RESOLVED**

**Files:**
- `src/db.rs` — `get_jobs_filtered()` query

**Severity:** TD

**Problem:**

Sorting jobs by file size used a correlated subquery in ORDER BY, executing one subquery per row and producing NULL for jobs without encode_stats.

**Fix:**

Added `LEFT JOIN encode_stats es ON es.job_id = j.id` to the base query. Sort column changed to `COALESCE(es.input_size_bytes, 0)`, ensuring jobs without stats sort as 0 (smallest) instead of NULL.

---

## Reliability Gaps

---

### [RG-1] No encode resume after crash or restart

**Status: PARTIALLY RESOLVED**

**Files:**
- `src/main.rs:320`
- `src/media/processor.rs:255`

**Severity:** RG

**Problem:**

Interrupted encodes were left in `Encoding` state and orphaned temp files remained on disk.

**Fix:**

Implemented `db.reset_interrupted_jobs()` in `main.rs` which resets `Encoding`, `Remuxing`, `Resuming`, and `Analyzing` jobs to `Queued` on startup. Orphaned temp files are also detected and removed. Full bitstream-level resume (resuming from the middle of a file) is still missing.

---

### [RG-2] AMD VAAPI/AMF hardware paths unvalidated

**Files:**
- `src/media/ffmpeg/vaapi.rs`
- `src/media/ffmpeg/amf.rs`

**Severity:** RG

**Problem:**

Hardware paths for AMD (VAAPI on Linux, AMF on Windows) were implemented without real hardware validation.

**Fix:**

Verify exact flag compatibility on AMD hardware and add integration tests gated on GPU presence.

---

## UX Gaps

---

### [UX-1] Queued jobs show no position or estimated wait time

**Status: RESOLVED**

**Files:**
- `src/db.rs` — `get_queue_position`
- `src/server/jobs.rs` — `JobDetailResponse.queue_position`
- `web/src/components/jobs/JobDetailModal.tsx`
- `web/src/components/jobs/types.ts` — `JobDetail.queue_position`

**Severity:** UX

**Problem:**

Queued jobs only show "Waiting" without indicating their position in the priority queue.

**Fix:**

Implemented `db.get_queue_position(job_id)` which counts jobs with higher priority or earlier `created_at` (matching the `priority DESC, created_at ASC` dequeue order). Added `queue_position: Option<u32>` to `JobDetailResponse` — populated only when `status == Queued`. Frontend shows `Queue position: #N` in the empty state card in `JobDetailModal`.

---

### [UX-2] No way to add a single file to the queue via the UI

**Severity:** UX

**Problem:**

Jobs only enter the queue via full library scans. No manual "Enqueue path" exists in the UI.

**Fix:**

Add `POST /api/jobs/enqueue` and a "Add file" action in the `JobsToolbar`.

---

### [UX-3] Workers-blocked reason not surfaced for queued jobs

**Severity:** UX

**Problem:**

Users cannot see why a job is stuck in Queued (paused, scheduled, or slots full).

**Fix:**

Add `GET /api/processor/status` and show the reason in the job detail.

---

## Feature Gaps

---

### [FG-4] Intelligence page content not actionable

**Files:**
- `web/src/components/LibraryIntelligence.tsx`

**Severity:** FG

**Problem:**

Intelligence page is informational only; recommendations cannot be acted upon directly from the page.

**Fix:**

Add "Queue all" for remux opportunities and "Review" actions for duplicates.

---

## What To Fix Next

1. **[UX-2]** Single file enqueue — New feature.
2. **[UX-3]** Workers-blocked reason — New feature.
3. **[FG-4]** Intelligence page actions — New feature.
4. **[RG-2]** AMD VAAPI/AMF validation — Needs real hardware.
