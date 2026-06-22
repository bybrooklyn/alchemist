# Audit Findings

Last updated: 2026-06-22

---

## P1 Issues

---

### [P1-1] Cancel during analysis can be overwritten by the pipeline

**Status: RESOLVED**

---

### [P1-2] VideoToolbox quality controls are effectively ignored

**Status: RESOLVED**

---

### [P1-3] Notification target migration rewrites the live table instead of evolving it additively

**Status: RESOLVED**

---

### [P1-4] Deleting one duplicate settings row could delete all matching rows

**Status: RESOLVED**

---

### [P1-5] Conversion expiry cleanup can delete active transcodes and their artifacts

**Status: RESOLVED**

---

### [P1-6] Manual conversion jobs silently fall back to library planning on conversion-row lookup failure

**Status: RESOLVED**

---

### [P1-7] Conversion start can queue an unlinked transcode that runs with library defaults

**Status: RESOLVED**

---

### [P1-8] Cancel and delete requests can poison future job runs with leaked cancel markers

**Status: RESOLVED**

---

### [P1-9] Race condition in job cancellation can lead to stuck "active" state

**Status: RESOLVED**

---

### [P1-10] Arbitrary file read via manual enqueue API

**Status: RESOLVED**

---

### [P1-11] `/api/jobs/clear-history` with unrecognised status deletes every job

**Status: RESOLVED**

**Files:**
- `src/db/jobs.rs:752–777` — `purge_jobs_by_filter` skips the `status IN (...)` clause when the supplied list is *empty*, not just when it is `None`.
- `src/server/jobs.rs:1074–1102` — `clear_history_handler` builds the list with `filter_map(|part| part.parse::<JobState>().ok())`, which silently drops unparsable tokens and can produce `Some(Vec::new())`.

**Severity:** P1

**Problem:**

`POST /api/jobs/clear-history?status=invalid` (or any combination where no token parses) results in `statuses = Some(vec![])`. `purge_jobs_by_filter` then does:

```rust
if let Some(st) = statuses {
    if !st.is_empty() { /* add status IN (...) */ }
    // empty -> no status filter added at all
}
```

The resulting SQL is `DELETE FROM jobs WHERE 1=1` (plus the optional `archived` filter). A single mistyped query parameter silently wipes the entire jobs table — including completed, queued, and active rows. Violates the binding "no data loss on failure" rule in DESIGN_PHILOSOPHY.md.

**Fix:**

1. In `src/db/jobs.rs::purge_jobs_by_filter`, treat empty `Some(vec)` as "match nothing":
   ```rust
   if let Some(st) = statuses {
       if st.is_empty() {
           return Ok(0); // caller asked for a status filter but supplied none
       }
       qb.push(" AND status IN (");
       /* bind each */
       qb.push(")");
   }
   ```
2. Tighten `clear_history_handler` to reject a `status` parameter that supplied at least one token but none parsed:
   ```rust
   let statuses = match params.status {
       Some(raw) => {
           let parsed: Vec<_> = raw.split(',')
               .filter_map(|part| part.parse::<JobState>().ok())
               .collect();
           if !raw.trim().is_empty() && parsed.is_empty() {
               return api_error_response(StatusCode::BAD_REQUEST,
                   "INVALID_STATUS_FILTER", "status filter contained no recognised values");
           }
           Some(parsed)
       }
       None => None,
   };
   ```
3. Add a regression test in `src/db/jobs.rs` mod tests that calls `purge_jobs_by_filter(Some(vec![]), None)` on a populated DB and asserts `0` rows affected and the jobs survive.

---

### [P1-12] Native mac app: SSE is dead in production — `AsyncBytes.lines` never yields the blank-line frame delimiter

**Status: RESOLVED** — SSE byte→line framing now lives in `AlchemistSSEParser`
(`consume(byte:)`/`consume(data:)`) so it survives `.lines` dropping the blank-line
delimiters; a defensive flush handles servers that omit them. `streamEvents` iterates
raw bytes, yields a synthetic `.connected` marker on a live 2xx response, and maps a 401
to `.unauthorized`. Check `sseParserSplitsFramesFromRawBytes` drives two complete frames
through the real byte path.

**Files:**
- `native/mac/Sources/AlchemistMacCore/API/AlchemistAPIClient.swift:377` — `for try await line in stream.lines` feeds the parser
- `native/mac/Sources/AlchemistMacCore/Models/AlchemistModels.swift:1301–1335` — `AlchemistSSEParser.parse(line:)` only flushes an event on an empty line
- `native/mac/Sources/AlchemistMacChecks/main.swift:60–76` — check passes by feeding `parse(line: "")` manually

**Severity:** P1

**Problem:**

SSE frames end with a blank line, and `AlchemistSSEParser` flushes the pending event only when `line.isEmpty`. But `URLSession.AsyncBytes.lines` (Foundation `AsyncLineSequence`) silently skips empty lines — verified empirically: piping `event: progress\n\ndata: x\n\nlast\n` through `FileHandle.bytes.lines` yields exactly 3 lines, no empties. In production the parser never sees the delimiter, so `eventName` is overwritten by each new `event:` line and `dataLines` accumulates JSON from multiple events (joined with `\n` → undecodable). Net effect: no progress, status, decision, log, engine, or config events ever reach the native UI. Combined with P2-38, the app's entire live-update layer is non-functional; `AlchemistMacChecks` passes because it bypasses the byte path.

**Fix:**

1. In `AlchemistAPIClient.streamEvents()`, stop using `.lines`. Iterate raw bytes and split manually, preserving empty lines:
   ```swift
   var parser = AlchemistSSEParser()
   var buffer: [UInt8] = []
   for try await byte in stream {
       if byte == UInt8(ascii: "\n") {
           var line = String(decoding: buffer, as: UTF8.self)
           if line.hasSuffix("\r") { line.removeLast() }
           buffer.removeAll(keepingCapacity: true)
           if let event = parser.parse(line: line) {
               continuation.yield(event)
           }
       } else {
           buffer.append(byte)
       }
   }
   ```
   Keep the existing `parser.finish()` at stream end.
2. Defensively, also flush the pending frame in `AlchemistSSEParser.parse` when a new `event:` line arrives while `eventName != nil || !dataLines.isEmpty` — return the flushed event before recording the new name.
3. Update `AlchemistMacChecks.sseParserUsesBackendEventNames` to drive a `Data` fixture of two complete frames through the real byte-splitting path, not hand-fed lines.
4. Verify with `just mac-run-bundled`: start an encode and confirm progress bars move without pressing Refresh.

---

### [P1-13] Native mac app: bundled daemon stdout/stderr pipes are never drained — daemon freezes once the 64 KB pipe buffer fills

**Status: RESOLVED** — stdout+stderr now stream to
`~/Library/Application Support/Alchemist/daemon.log` (`FileHandle`, no undrained
`Pipe()`), so the daemon can't block once the buffer fills. `recentLogLines()` exposes
the tail (surfaced on start/crash failures) and the log path is shown in SystemView's
Host Paths card.

**Files:**
- `native/mac/Sources/AlchemistMacCore/Daemon/DaemonController.swift:52–53` — `process.standardOutput = Pipe(); process.standardError = Pipe()` with no reader

**Severity:** P1

**Problem:**

Two `Pipe()`s are attached and never read. Once alchemistd (running with `RUST_LOG=info`) has written ~64 KB of log output, its next `write(2)` to stdout blocks forever — a library scan or a few encodes is enough. The daemon then hangs mid-operation (jobs stuck in `encoding`, API unresponsive) and the app shows no error. This is a guaranteed-eventual hang, not a race.

**Fix:**

1. In `startBundledDaemon`, drain both pipes. Simplest robust option — stream to a log file under Application Support:
   ```swift
   let logURL = AlchemistSupportPaths.root.appendingPathComponent("daemon.log")
   FileManager.default.createFile(atPath: logURL.path, contents: nil)
   let handle = try FileHandle(forWritingTo: logURL)
   process.standardOutput = handle
   process.standardError = handle
   ```
   Or keep `Pipe()` and attach `readabilityHandler`s appending to a ring buffer surfaced in SystemView (better diagnostics; prefer this if cheap).
2. Expose the last N stderr lines via `DaemonController` so bind failures (see P2-40) become visible.
3. Add the log path to SystemView's "Host Paths" card.

---

### [P1-14] VideoToolbox is reported "available" by a probe that allows software fallback, but the real encode forbids it and requests an unsupported `-q:v` mode — every hardware encode fails on session-less hosts, then retries forever

**Status: RESOLVED (2026-06-22).** Probe honesty: `probe_args_for_backend` no longer
passes `-allow_sw 1`, so detection matches the real (hardware-only) command and VT is not
selected when no hardware session can open (`hardware.rs`, test
`videotoolbox_probe_is_hardware_only_and_matches_real_command`). One-time runtime CPU
fallback: on an encoder-open FFmpeg failure the pipeline re-plans onto the CPU encoder
(`cpu_fallback_plan`) and re-runs once before failing (`pipeline.rs`, tests
`cpu_fallback_plan_swaps_hardware_encoder_for_cpu_crf`,
`encoder_cpu_equivalent_and_is_hardware`). Reclassification: `map_failure` now maps
encoder-open failures to `EncoderUnavailable` (not `Transient`) via
`explanations::is_encoder_open_failure` (new `encoder_open_failed` stderr signature), so
they stop retrying and surface a coded, docs-linked explanation. The misleading `-q:v`
clamp comment was corrected.

**Files:**
- `src/system/hardware.rs:407–417` — the capability probe adds `-vf format=yuv420p`, **`-allow_sw 1`**, and no rate-control flag, then encodes one frame.
- `src/media/ffmpeg/videotoolbox.rs:3–47` — `append_args` emits `-c:v hevc_videotoolbox` + `-q:v <n>` (constant quality) and **never emits `-allow_sw`**.
- `src/media/pipeline.rs:2552–2558` — `map_failure` maps any `AlchemistError::FFmpeg(_)` to `JobFailure::Transient` (catch-all `_ =>`).

**Severity:** P1

**Problem:**

Reproduced on this host (Apple M4, arm64) and matches the user's daemon log (`hevc_videotoolbox … Could not open encoder before EOF … error code: -22 (Invalid argument)`, job retried as `Transient`). Three compounding defects:

1. **Detection/runtime mismatch.** The probe validates VideoToolbox *with software fallback enabled* (`-allow_sw 1`) and no rate control, so it succeeds even in a context that cannot create a hardware VT session (LaunchDaemon / SSH / headless / sandboxed). The real encode command (`videotoolbox.rs`) **omits `-allow_sw`**, so it requires a hardware session. On any host where the daemon runs without GUI/WindowServer access, detection reports "VideoToolbox available", the planner selects `hevc_videotoolbox`, and **every** encode then dies with `-22`. Verified directly:
   ```
   probe   : ffmpeg … -c:v hevc_videotoolbox -allow_sw 1 -frames:v 1 -f null -   → exit 0 (PASS)
   encode  : ffmpeg … -c:v hevc_videotoolbox -q:v 28 -f null -                   → "Conversion failed!"
   ```
2. **Unsupported constant-quality mode.** Even with `-allow_sw 1`, the software VideoToolbox encoder rejects `-q:v` (constant quality): `-allow_sw 1 -q:v 28` fails, while `-allow_sw 1 -b:v 1500k` succeeds (25 frames). The CQ path introduced by the P1-2 fix produces a command this build/encoder cannot open; bitrate mode is the working path. The code comment ("VideoToolbox -q:v: 1 (best) to 100 (worst)") is also inaccurate.
3. **Misclassified as Transient → infinite retry, no CPU fallback.** A VideoToolbox "Could not open encoder / Invalid argument" is deterministic — the identical command fails identically on every attempt. `map_failure`'s catch-all marks it `Transient`, so the job is re-queued and re-run forever instead of either falling back to CPU (the project's stated "hardware acceleration … with CPU fallback") or being marked a permanent encoder failure.

Net user-visible effect: on an affected host the entire library "encodes" but produces nothing, the queue churns failed→queued→failed, and the logs fill with `-22`. This breaks the core transcode promise and contradicts the "deterministic behavior / explicit error handling over implicit fallbacks" design rule.

**Fix:**

1. Make detection represent the real command. Either drop `-allow_sw 1` from `probe_args_for_backend` (so the probe fails exactly when the production encoder would and the planner correctly falls back to CPU), **or** add `-allow_sw 1` to the real `videotoolbox.rs` command so software VT is a legal fallback. Recommend the former for hardware-accel hosts and exposing the latter as an explicit "allow software VideoToolbox" setting — but the two must agree.
2. In `src/media/ffmpeg/videotoolbox.rs::append_args`, stop emitting constant-quality `-q:v` for VideoToolbox unless validated on the target. Default to bitrate-based rate control (`-b:v`/`-maxrate`/`-bufsize`, already implemented in the `RateControl::Bitrate` arm) derived from the CRF target, and only keep `-q:v` behind a capability check that actually opened a CQ session during probing. Fix the misleading scale comment.
3. In `src/media/pipeline.rs`, detect encoder-open failures from FFmpeg stderr ("Could not open encoder", "Error while opening encoder", "Invalid argument" at session create) and route them to a non-Transient outcome: trigger the planned CPU fallback if `allow_fallback`, else classify `JobFailure::EncoderUnavailable` so the job stops retrying and the UI surfaces an actionable reason.
4. Add a probe-time CQ validation: when constant-quality is requested for VideoToolbox, run a one-frame `-q:v` probe and cache whether it opened; only emit `-q:v` when it did.
5. Tests: a unit test asserting probe args and real-encode args use the same `-allow_sw` policy; a `map_failure`/stderr-classifier test that an encoder-open failure is not `Transient`.

---

## P2 Issues

---

### [P2-1] Convert does not reuse subtitle/container compatibility checks

**Status: RESOLVED**

---

### [P2-2] Completed job metadata omitted at the API layer

**Status: RESOLVED**

---

### [P2-3] LAN-only setup exposed to reverse proxy misconfig

**Status: RESOLVED**

---

### [P2-4] N+1 DB update in batch cancel

**Status: RESOLVED**

---

### [P2-5] Missing archived filter in health and stats queries

**Status: RESOLVED**

---

### [P2-6] Daily summary notifications bypass SSRF protections

**Status: RESOLVED**

---

### [P2-7] Silent reprobe failure corrupts saved encode stats

**Status: RESOLVED**

---

### [P2-8] Finalization reprobes the input file instead of the encoded output

**Status: RESOLVED**

---

### [P2-9] Job detail handler turns database failures into empty sections and still returns 200

**Status: RESOLVED**

---

### [P2-10] `%` and `_` in watch folder paths can assign the wrong library profile

**Status: RESOLVED**

---

### [P2-11] Login collapses database errors into “invalid credentials”

**Status: RESOLVED**

---

### [P2-12] Job SSE reconciliation leaves filtered tables and the detail modal stale

**Status: RESOLVED**

---

### [P2-13] Conversion upload buffers the entire video into memory

**Status: RESOLVED**

---

### [P2-14] Conversion preview can return 200 even when the saved settings were not persisted

**Status: RESOLVED**

---

### [P2-15] Engine mode requests can fail persistently but still change the live runtime

**Status: RESOLVED**

---

### [P2-16] Auth middleware turns session and API-token database failures into fake 401s

**Status: RESOLVED**

---

### [P2-17] Bulk watch-dir sync can persist a broken config even when the request returns an error

**Status: RESOLVED**

---

### [P2-18] Profile lookup failures still produce authoritative decisions and intelligence recommendations

**Status: RESOLVED**

---

### [P2-19] Deleting a conversion can report success even when the linked job was never archived

**Status: RESOLVED**

---

### [P2-20] Library intelligence endpoint performs unbounded N+1 planning work on every request

**Status: RESOLVED**

---

### [P2-21] Browser-side backup download defeats the backend’s streaming snapshot path

**Status: RESOLVED**

---

### [P2-23] Library reanalyze handler performs unbounded row loading

**Status: RESOLVED**

---

### [P2-24] Library health scan flag plumbed but never consulted (regression of RG-5)

**Status: RESOLVED**

**Files:**
- `src/server/scan.rs:218–231` — `start_library_health_scan_handler` spawns an unguarded task.
- `src/server/mod.rs:172`, `:202`, `:295`, `:375` — `library_health_scan_in_progress: Arc<AtomicBool>` is allocated and threaded into `AppState` but never `.load()`-ed or `.store()`-ed anywhere except in test setup.
- `src/main.rs:997` — created and seeded `false`, never written.

**Severity:** P2

**Problem:**

RG-5 (resolved 2026-04-30) was "Library health scan endpoint allows overlapping full-library runs". The fix introduced `library_health_scan_in_progress: AtomicBool` and threaded it through `AppState`, but `start_library_health_scan_handler` does not consult it. The atomic is plumbed but inert. Two simultaneous `POST /api/library/health/scan` calls each spawn a `run_library_health_scan` task; both call `db.create_health_scan_run()` and then crawl every health-eligible file concurrently. Disk I/O contention can starve the encode pipeline, and double-counted health rows are written.

```rust
pub(crate) async fn start_library_health_scan_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = state.db.clone();
    tokio::spawn(async move {
        run_library_health_scan(db).await;
    });
    (StatusCode::ACCEPTED, axum::Json(serde_json::json!({ "status": "accepted" })))
        .into_response()
}
```

No `compare_exchange`, no early-return path.

**Fix:**

1. In `src/server/scan.rs::start_library_health_scan_handler`, guard with the atomic:
   ```rust
   if state.library_health_scan_in_progress
       .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
       .is_err()
   {
       return api_error_response(StatusCode::CONFLICT,
           "HEALTH_SCAN_IN_PROGRESS",
           "A library health scan is already running");
   }
   let db = state.db.clone();
   let flag = state.library_health_scan_in_progress.clone();
   tokio::spawn(async move {
       run_library_health_scan(db).await;
       flag.store(false, Ordering::SeqCst);
   });
   ```
2. Use a small RAII guard so the flag clears even if the task panics. The existing `run_library_health_scan` already wraps its body in `AssertUnwindSafe.catch_unwind()`, but the wrapper that clears the flag must run regardless of panic.
3. Add an integration test in `src/server/tests.rs` that fires two POSTs and asserts the second returns 409 with `HEALTH_SCAN_IN_PROGRESS`.

---

### [P2-25] `get_stats` returns counts that include archived jobs

**Status: RESOLVED**

**Files:**
- `src/db/stats.rs:29–47` — `get_stats` queries `FROM jobs GROUP BY status` with no `WHERE archived = 0` clause.
- `src/server/stats.rs:22–63` — `get_stats_data` / `stats_handler` consume the result as `total`/`active`/`failed`/`completed`.
- `src/db/stats.rs:368–435` — `get_daily_summary_stats` shares the same omission for its `CASE WHEN` aggregation.

**Severity:** P2

**Problem:**

P2-5 (resolved) explicitly added `archived = 0` filters to health and other stats queries. The base `get_stats` query was missed:

```rust
let stats = sqlx::query("SELECT status, count(*) as count FROM jobs GROUP BY status")
```

The dashboard "Total Jobs", "Completed", "Failed" cards include rows the user cleared via *Clear completed* (which archives rather than deletes). After running the queue for a few weeks the displayed total drifts arbitrarily upward and never matches the table the user can actually see, which is filtered to `archived = 0` by `jobs_table_handler`. The same omission in `get_daily_summary_stats` makes daily notification emails over-count.

**Fix:**

1. In `src/db/stats.rs::get_stats`, mirror `get_job_stats`:
   ```rust
   let stats = sqlx::query(
       "SELECT status, count(*) as count FROM jobs WHERE archived = 0 GROUP BY status",
   )
   ```
2. In `src/db/stats.rs::get_daily_summary_stats`, add the same `WHERE archived = 0` to the outer aggregation over `FROM jobs`.
3. Extend the existing `get_aggregated_stats_excludes_archived_jobs` test in `src/db/stats.rs` so it also asserts `get_stats` drops archived rows from its map.

---

### [P2-26] Concurrent install_system_update can race two update helpers

**Status: RESOLVED**

**Files:**
- `src/server/system.rs:645–764` — `install_system_update_handler` has no mutex protecting the stage/spawn/exit sequence.
- `src/update.rs:269–302` — `spawn_update_helper` forks a shell script and exits the parent 750ms later.

**Severity:** P2

**Problem:**

Two simultaneous `POST /api/system/update/install` calls from the same operator (impatient click, retry after spinner timeout) each call `stage_update_asset`, `create_update_backup`, and `spawn_update_helper`. Both spawn shell helpers that wait for *this* process to exit and then `mv` the staged binary into place. The second helper races the first: it can move the in-flight `.failed-update` rollback back over the freshly written binary, or both fight for the same `current` path.

`tokio::spawn(async { sleep(750ms); exit(0); })` is racy on its own — the second call still proceeds for ~750 ms before `process::exit(0)` fires.

**Fix:**

1. Add `update_install_in_progress: Arc<AtomicBool>` to `AppState` (mirror `library_health_scan_in_progress`).
2. Gate `install_system_update_handler`:
   ```rust
   if state.update_install_in_progress
       .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
       .is_err()
   {
       return api_error_response(StatusCode::CONFLICT,
           "UPDATE_INSTALL_IN_PROGRESS",
           "An update install is already in progress");
   }
   ```
3. Clear the flag on every early-return path (drain pending, asset/version unavailable, stage failure, helper spawn failure). A small RAII guard struct that clears on `Drop` is the cleanest implementation.

---

### [P2-27] reanalyze_library_root_handler silently swallows per-root DB errors

**Status: RESOLVED**

---

### [P2-28] PERF-3 probe cache stores `file_id` but never verifies it on read

**Status: RESOLVED** — `get_media_probe_cache_with_file_id` now compares the stored hint; `analyze_with_cache` passes the current `file_id`. Half-known identity is treated as a miss. Tests `file_id_mismatch_is_treated_as_cache_miss` and `legacy_rows_without_file_id_still_hit` added. Follow-up from codex review: the Unix `file_id` now encodes `dev:<dev>:ino:<ino>` (full POSIX file identity) rather than `ino` alone, since inode numbers are only unique within one filesystem.

**Files:**
- `src/media/analyzer.rs:345–361` — `probe_cache_key_for_path` computes the inode/volume hint into `ProbeCacheKey.file_id`.
- `src/media/analyzer.rs:363–433` — `analyze_with_cache` writes `file_id` on miss (line 414) but the lookup at line 369–386 only passes `(input_path, mtime_ns, size_bytes, probe_version)`.
- `src/db/probe_cache.rs:6–46` — `get_media_probe_cache` SQL `WHERE input_path = ? AND mtime_ns = ? AND size_bytes = ? AND probe_version = ?` — no `file_id` clause.
- `migrations/20260513120100_watch_dirs_last_scanned_and_probe_cache_file_id.sql:8–12` — adds the column the lookup never consults.

**Severity:** P2

**Problem:**

CHANGELOG.md for 0.3.2-rc.3 advertises PERF-3 as: "re-analysis is skipped when path, size, mtime, and inode/volume index all match." In code, the inode/volume part is write-only — the analyzer computes `file_id` and stores it on cache miss, but neither the cache key nor the read path ever compares it against the stored value. The behavior is identical to the pre-PERF-3 design.

```rust
// analyze_with_cache, src/media/analyzer.rs
db.get_media_probe_cache(
    &cache_key.input_path,
    cache_key.mtime_ns,
    cache_key.size_bytes,
    &cache_key.probe_version,
)
// ↑ no file_id passed; cache_key.file_id is only used for the write below.
```

```sql
-- get_media_probe_cache
SELECT analysis_json
  FROM media_probe_cache
 WHERE input_path = ? AND mtime_ns = ? AND size_bytes = ? AND probe_version = ?
```

The doc comment on `upsert_media_probe_cache_with_file_id` (probe_cache.rs:67–71) explicitly states: *"callers can verify that path+size+mtime really match the same on-disk object instead of a replaced inode that happens to share metadata."* The only caller (`analyze_with_cache`) never performs that verification.

Failure mode this leaves on the table: when a file is replaced atomically (rsync `--inplace`, snapshot restore, hardlink swap) and the new content happens to share path + size + mtime second-precision with the prior file, the analyzer returns the stale analysis instead of re-probing — which is the exact scenario PERF-3 documented as the reason for storing the file_id.

**Fix:**

1. Extend `get_media_probe_cache` to accept and verify the optional `file_id`. Treat `(stored=None, supplied=Some)` and `(stored=Some, supplied=None)` as cache misses, because we can't prove identity matches with only one half:
   ```rust
   pub async fn get_media_probe_cache_with_file_id(
       &self,
       input_path: &str,
       mtime_ns: i64,
       size_bytes: i64,
       probe_version: &str,
       file_id: Option<&str>,
   ) -> Result<Option<String>> {
       let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
           "SELECT analysis_json, file_id
              FROM media_probe_cache
             WHERE input_path = ? AND mtime_ns = ? AND size_bytes = ? AND probe_version = ?",
       )
       .bind(input_path).bind(mtime_ns).bind(size_bytes).bind(probe_version)
       .fetch_optional(&self.pool)
       .await?;
       let Some((json, stored_fid)) = row else { return Ok(None) };
       match (stored_fid.as_deref(), file_id) {
           (Some(a), Some(b)) if a != b => Ok(None),       // identity changed
           (Some(_), None) | (None, Some(_)) => Ok(None),  // half-known → don't trust
           _ => Ok(json),                                  // both None or both matching
       }
       // touch last_accessed_at as before
   }
   ```
2. In `analyze_with_cache`, call the new function and pass `cache_key.file_id.as_deref()`. Keep `get_media_probe_cache` as a thin shim that calls the new one with `file_id = None` for callers that genuinely don't have one.
3. Add a `probe_cache.rs` unit test: insert a row with `file_id = Some("ino:1")`, query with `file_id = Some("ino:2")`, assert `None`; query with `Some("ino:1")` asserts `Some(json)`.

---

### [P2-29] `clear_media_probe_cache_under` triggers undefined LIKE behavior on Windows paths

**Status: RESOLVED** — the prefix is now `\`/`%`/`_`-escaped before binding, mirroring `db/jobs.rs` search escaping. Test `clear_under_prefix_handles_windows_and_wildcard_paths` covers Windows backslash prefixes and literal underscores.

**Files:**
- `src/db/probe_cache.rs:110–133` — `clear_media_probe_cache_under` binds raw Windows paths into a LIKE pattern with `ESCAPE '\'`.
- `src/system/scanner.rs:103–121` — `start_scan_with_options(force_full = true)` is the production caller; every Windows force-full scan exercises this path.

**Severity:** P2

**Problem:**

```rust
let mut like_pattern = normalized.clone();
like_pattern.push('%');

let result = sqlx::query(
    "DELETE FROM media_probe_cache
     WHERE input_path = ? OR input_path LIKE ? ESCAPE '\\'",
)
.bind(path_prefix)
.bind(like_pattern)
```

The escape character is `\`. The `like_pattern` is the path itself with no pre-escaping. On Windows the path is `C:\Users\me\library\%`. SQLite docs state: *"If the escape character is followed by any other character, the result of the LIKE function is undefined."* That is exactly what happens here — every `\` in the path is followed by an arbitrary alphanumeric.

SQLite's current implementation tolerantly treats `\X` (X not in `_`/`%`/`\`) as literal `X`, so the pattern silently becomes the literal string `C:Usersmelibrary` followed by a literal `%` (because the final `\%` strips the wildcard meaning). That string matches no real cached row, so on Windows the force-full scan's "wipe probe cache under this prefix" step is a no-op. Result: force-full scans on Windows do not refresh probe data, defeating the only documented purpose of the button.

`src/db/jobs.rs:572–580` already shows the correct pattern (manual pre-escape of `\`, `%`, `_` before binding with `ESCAPE '\\'`). The cache wipe was missed.

**Fix:**

1. Mirror the search-query escaping in `clear_media_probe_cache_under`:
   ```rust
   pub async fn clear_media_probe_cache_under(&self, path_prefix: &str) -> Result<u64> {
       let normalized = /* unchanged */;
       let escaped = normalized
           .replace('\\', "\\\\")
           .replace('%', "\\%")
           .replace('_', "\\_");
       let like_pattern = format!("{}%", escaped);
       let result = sqlx::query(
           "DELETE FROM media_probe_cache
            WHERE input_path = ? OR input_path LIKE ? ESCAPE '\\'",
       )
       .bind(path_prefix)
       .bind(like_pattern)
       .execute(&self.pool)
       .await?;
       Ok(result.rows_affected())
   }
   ```
2. Add a unit test that inserts cache rows under `C:\\Users\\me\\movies\\a.mkv` and `C:\\Users\\me\\music\\b.mp3`, calls `clear_media_probe_cache_under("C:\\Users\\me\\movies")`, and asserts the movies row was deleted while the music row survived.
3. Add a second test using an Unix prefix containing a literal `_` (e.g. `/media/season_01/`) to lock the behavior that `_` in the prefix is no longer treated as a single-character wildcard.

---

### [P2-30] `/api/v1/library/preview` runs up to 2000 ffprobe + N+1 DB queries per request, unbounded by rate limit

**Status: RESOLVED** — added a single-flight `library_preview_in_progress` atomic (concurrent previews now return `429 PREVIEW_BUSY`), lowered the cap from 2000 to 200 (default 60), and now reject paths outside configured library/watch roots with `403 PREVIEW_PATH_FORBIDDEN`. Test `library_preview_rejects_paths_outside_configured_roots` added. The bounded-parallelism / profile-batch optimisation (fix steps 3–4) was left out as a non-correctness optimisation — single-flight + 200-cap already bounds load.

**Files:**
- `src/server/scan.rs:139–301` — `preview_library_path_handler` walks the supplied directory, serially calls `analyze_with_cache` and `get_profile_for_path` per file up to `max_files` (clamped 1–2000).
- `src/server/mod.rs:879` — route registration; auth-gated but no per-route rate cap.
- `web/src/components/WatchFolders.tsx:209–227` — UI invokes the endpoint on every Eye-icon click.

**Severity:** P2

**Problem:**

The preview handler does its work synchronously inside the request:

```rust
for discovered in &to_process {
    // skip_reason_for_discovered_path → DB query
    // analyzer.analyze_with_cache → ffprobe subprocess (cold) or DB read
    // db.get_profile_for_path → DB query
    // planner.plan → CPU work
}
```

On the first preview of a 2000-file directory every ffprobe is a cache miss. Per-file probe cost is 50 ms – 1 s; total wall time 100 s – 2000 s. The request hangs that long, the browser sees a stalled response, and one authenticated client can pin a worker indefinitely by spamming the endpoint. There is no shared lock, no rate limit on the handler, and no cap on concurrent in-flight previews.

The N+1 profile query and skip-reason query inside the loop compound the SQLite pool contention; the rest of the app's handlers slow down measurably while a preview is in flight.

Additionally, `preview_root` is validated only with `exists()` + `is_dir()` — there is no check that the supplied path lies inside a configured watch root or library directory. The host already exposes `fs_browse`, so this isn't a new disclosure surface, but it does mean an attacker with a stolen UI session can scan `/` and force the host to ffprobe every video on the filesystem.

**Fix:**

1. Cap concurrent previews with an `Arc<tokio::sync::Semaphore>` in `AppState` (size 1 is fine — this is an interactive feature, not a throughput one). Return `429 Too Many Requests` with code `PREVIEW_BUSY` when the semaphore is full.
2. Lower `PREVIEW_DEFAULT_MAX_FILES` to something realistic (e.g. 60) and clamp the max to 200, not 2000. The samples list is already capped at 20 — 200 plan executions is plenty to give the user a representative summary.
3. Run probes concurrently with bounded parallelism using `futures::stream::iter(files).for_each_concurrent(4, ...)`. Cold previews of small folders then complete in seconds instead of minutes.
4. Batch the profile lookup: call `state.db.get_profiles_for_paths(&paths)` once (new method) and look up per-file in a `HashMap`. Eliminates the N+1.
5. Validate that `preview_root` canonicalizes inside one of `config.scanner.directories ∪ db.get_watch_dirs()` before doing any work, returning `403 FORBIDDEN` with code `PREVIEW_PATH_FORBIDDEN` otherwise. Mirrors the rule `enqueue_job_from_submitted_path` already enforces (jobs.rs:145–178).
6. Add an integration test that calls the endpoint twice concurrently and asserts the second receives 429.

---

### [P2-31] Decisions and failure-explanation trends don't filter archived jobs

**Status: RESOLVED** — every `decisions`/`job_failure_explanations` reason query in `db/stats.rs` (counts, windowed counts, both trend queries, and the two `get_daily_summary_stats` subqueries) now `JOIN jobs … WHERE archived = 0`. Test `reason_trends_exclude_archived_jobs` added.

**Files:**
- `src/db/stats.rs:440–464` — `get_skip_reason_counts` queries `decisions` with no join to `jobs`.
- `src/db/stats.rs:468–500` — `get_skip_reason_counts_windowed` ditto.
- `src/db/stats.rs:505–533` — `get_failure_code_counts` queries `job_failure_explanations` directly.
- `src/db/stats.rs:541–565` — `get_skip_reason_trend`, same omission.
- `src/db/stats.rs:567–590` — `get_failure_code_trend`, same omission.
- `src/db/stats.rs:397–426` — `get_daily_summary_stats` top-failure and top-skip subqueries.

**Severity:** P2

**Problem:**

P2-25 (resolved 2026-05-12) added `WHERE archived = 0` to `get_stats` and the main `get_daily_summary_stats` aggregation so cleared rows don't keep inflating the "Total Jobs / Completed / Failed" cards. The fix did not extend to the **reason-code** family of queries. After the user clicks *Clear completed* (which archives the rows), the Statistics page's "Top Skip Reasons" and "Top Failure Reasons" tables, plus the sparkline trends behind them, still count the archived rows because both `decisions` and `job_failure_explanations` are queried without a `jobs.archived = 0` filter:

```sql
-- get_failure_code_counts
SELECT code, COUNT(*) AS count, MAX(updated_at) AS last_seen
FROM job_failure_explanations
WHERE updated_at >= datetime('now', ?)
GROUP BY code
ORDER BY count DESC, code ASC
LIMIT 20
```

```sql
-- get_skip_reason_counts (today's bucket)
SELECT COALESCE(reason_code, action) AS code, COUNT(*) AS count
FROM decisions
WHERE action = 'skip' AND created_at >= datetime('now', 'start of day', 'localtime')
GROUP BY COALESCE(reason_code, action)
ORDER BY count DESC, code ASC
LIMIT 20
```

The daily summary notification suffers the same inconsistency: the outer counts respect archived, but the embedded "Top failure reasons" / "Top skip reasons" lists do not.

User-visible effect: after clearing history, the dashboard tiles drop to the right values but the reason tables/sparklines stay at their pre-clear numbers. That's the regression P2-25 was meant to remove.

**Fix:**

1. Add `INNER JOIN jobs j ON j.id = decisions.job_id AND j.archived = 0` (or `EXISTS` equivalent) to every `decisions`-based query in `src/db/stats.rs`. Specifically:
   ```sql
   SELECT COALESCE(d.reason_code, d.action) AS code, COUNT(*) AS count
   FROM decisions d
   JOIN jobs j ON j.id = d.job_id
   WHERE d.action = 'skip'
     AND d.created_at >= datetime('now', ?)
     AND j.archived = 0
   GROUP BY COALESCE(d.reason_code, d.action)
   ```
2. Do the same for `get_failure_code_counts` and `get_failure_code_trend` against `job_failure_explanations`. Note: `job_failure_explanations.job_id` is unique per job (ON CONFLICT upsert), so a normal INNER JOIN is correct and won't double-count.
3. Apply the join inside the two embedded subqueries of `get_daily_summary_stats` (lines 397–406 and 412–422). The outer aggregation already filters archived rows; the subqueries must too.
4. Watch the indexes: the existing `idx_decisions_created_at_action` and `idx_failure_explanations_updated_at` already help the WHERE clauses; the join adds a single index seek on `jobs(id)` (primary key) per row. No new index needed.
5. Extend the existing `get_aggregated_stats_excludes_archived_jobs` test in `src/db/stats.rs` (or add a sibling test) that creates a skip + failure for an archived job and asserts both `get_skip_reason_counts_windowed` and `get_failure_code_counts` exclude it.

---

### [P2-32] Notification SSRF guard misses IPv4-mapped IPv6 addresses

**Status: RESOLVED** — `is_private_ip` now normalizes IPv4-mapped IPv6 via `to_ipv4_mapped()` and recurses into the IPv4 classifier. Test `ipv4_mapped_ipv6_internal_addresses_classify_as_private` covers metadata/loopback/RFC1918 mapped addresses plus a mapped public address.

**Files:**
- `src/notifications.rs:1092–1110` — `is_private_ip` checks `IpAddr::V6` against `is_loopback`/`is_unique_local`/`is_unicast_link_local`/`is_multicast`/`is_unspecified` but never normalizes IPv4-mapped IPv6 (`::ffff:a.b.c.d`).
- `src/notifications.rs:181–226` — `build_safe_client` resolves the endpoint host, picks the first IP for which `!is_private_ip`, and pins it with `.resolve(...)`.

**Severity:** P2

**Problem:**

P2-6 (resolved 2026-04-30) added SSRF protection to notification delivery: resolve the host, reject private IPs, pin the resolved address to defeat DNS rebinding. The IPv6 branch of `is_private_ip` does not account for IPv4-mapped IPv6 addresses:

```rust
IpAddr::V6(v6) => {
    v6.is_loopback()
        || v6.is_unique_local()
        || v6.is_unicast_link_local()
        || v6.is_multicast()
        || v6.is_unspecified()
}
```

`::ffff:169.254.169.254` (the cloud metadata endpoint expressed as an IPv4-mapped IPv6 address) returns `false` from every one of these predicates — `is_unicast_link_local()` only matches `fe80::/10`, not the mapped form. So a notification webhook URL of `http://[::ffff:169.254.169.254]/latest/meta-data/...`, or a hostname whose AAAA record a malicious DNS server points at a mapped internal address, passes the "public IP" filter. The address is then pinned with `.resolve()` and the OS routes `::ffff:x` straight to the IPv4 destination — reaching link-local, loopback, or RFC1918 services the guard was meant to block.

The same gap affects `::ffff:127.0.0.1`, `::ffff:10.x`, `::ffff:192.168.x`, etc.

**Fix:**

1. Normalize IPv4-mapped IPv6 to IPv4 before classifying:
   ```rust
   fn is_private_ip(ip: IpAddr) -> bool {
       match ip {
           IpAddr::V4(v4) => {
               v4.is_private() || v4.is_loopback() || v4.is_link_local()
                   || v4.is_multicast() || v4.is_unspecified() || v4.is_broadcast()
           }
           IpAddr::V6(v6) => {
               if let Some(v4) = v6.to_ipv4_mapped() {
                   return is_private_ip(IpAddr::V4(v4));
               }
               v6.is_loopback() || v6.is_unique_local()
                   || v6.is_unicast_link_local() || v6.is_multicast()
                   || v6.is_unspecified()
           }
       }
   }
   ```
   Note: prefer `to_ipv4_mapped()` over `to_ipv4()` — the latter also converts deprecated IPv4-compatible addresses and `::1`, which is acceptable here but `to_ipv4_mapped()` is the precise check.
2. Add a unit test covering `::ffff:169.254.169.254`, `::ffff:127.0.0.1`, and `::ffff:10.0.0.1` all classifying as private, and a normal mapped public address (`::ffff:8.8.8.8`) classifying as public.
3. Audit any other consumer of `is_private_ip` (currently only `build_safe_client`) for the same assumption.

---

### [P2-33] Rate Limiting DOS via Reverse Proxy IP in Login Handler

**Status: RESOLVED in 0.3.4-rc.2.** Login and global rate limiting now use the
shared trusted-proxy-aware client-IP resolver, with regression coverage for
spoofed headers and independent proxied-client login buckets.

**Files:**
- `src/server/auth.rs:26–37` — `login_handler` fetches client IP using `ConnectInfo(addr)`.
- `src/server/middleware.rs:346–370` — `allow_login_attempt` uses the direct socket peer IP.

**Severity:** P2

**Problem:**

`login_handler` relies on Axum's `ConnectInfo<SocketAddr>` to extract the client IP for rate limiting via `addr.ip()`. However, if Alchemist is deployed behind a TLS-terminating reverse proxy (e.g., Nginx, Caddy, or Traefik), `addr.ip()` will always resolve to the proxy's IP (e.g., `127.0.0.1` or the Docker gateway). If an attacker spams invalid login attempts, the proxy's IP gets rate-limited, locking out *all* legitimate users from logging in (Denial of Service).

```rust
pub(crate) async fn login_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::Json(payload): axum::Json<LoginPayload>,
) -> impl IntoResponse {
    if !allow_login_attempt(&state, addr.ip()).await {
        return api_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "AUTH_RATE_LIMITED",
            "Too many requests",
        );
    }
```

**Fix:**

1. In `src/server/auth.rs`, modify `login_handler` to accept `axum::http::HeaderMap` and extract the resolved client IP using `is_trusted_peer` validation:
   ```rust
   pub(crate) async fn login_handler(
       State(state): State<Arc<AppState>>,
       ConnectInfo(addr): ConnectInfo<SocketAddr>,
       headers: axum::http::HeaderMap,
       axum::Json(payload): axum::Json<LoginPayload>,
   ) -> impl IntoResponse {
       let mut client_ip = addr.ip();
       if super::middleware::is_trusted_peer(client_ip, &state.trusted_proxies) {
           if let Some(xff) = headers.get("X-Forwarded-For") {
               if let Ok(xff_str) = xff.to_str() {
                   if let Some(ip_str) = xff_str.split(',').next() {
                       if let Ok(ip) = ip_str.trim().parse() {
                           client_ip = ip;
                       }
                   }
               }
           } else if let Some(xri) = headers.get("X-Real-IP") {
               if let Ok(xri_str) = xri.to_str() {
                   if let Ok(ip) = xri_str.trim().parse() {
                       client_ip = ip;
                   }
               }
           }
       }

       if !allow_login_attempt(&state, client_ip).await {
           return api_error_response(
               StatusCode::TOO_MANY_REQUESTS,
               "AUTH_RATE_LIMITED",
               "Too many requests",
           );
       }
   ```
2. In `src/server/tests.rs`, add an integration test that fires request scenarios with proxy headers to confirm client IP separation works correctly.

---

### [P2-34] Batch delete/restart half-applies the mutation, then returns 409 — leaking resume sessions and silently requeuing jobs

**Status: RESOLVED** — `batch_jobs_handler` now de-duplicates the requested ids and rejects the batch with `409 BATCH_ACTION_CONFLICT` *before* any DB mutation when the eligible-row fetch (`get_jobs_by_ids`, which already filters `archived = 0`) returns fewer rows than the deduped id set. The resume-session purge runs over the deduped set on the delete path. `batch_delete_jobs`/`batch_restart_jobs` additionally gained `archived = 0 AND status NOT IN ('analyzing','encoding','remuxing','resuming')` guards. Test `test_batch_mutation_safety_predicates` added.

**Files:**
- `src/server/jobs.rs:491–503` — `batch_jobs_handler` delete/restart arm returns `409 BATCH_ACTION_CONFLICT` when `count as usize != payload.ids.len()` *after* the DB mutation has already committed, and before `purge_resume_sessions_for_jobs`.
- `src/db/jobs.rs:677–711` — `batch_delete_jobs` / `batch_restart_jobs` now skip rows where `archived = 1` or status is active, so `count` is legitimately smaller than `ids.len()` for stale ids.
- `src/server/jobs.rs:393–397` — `BatchActionPayload.ids: Vec<i64>` is never de-duplicated.

**Severity:** P2

**Problem:**

The new safety predicates on `batch_delete_jobs`/`batch_restart_jobs` (added in this change) make `count` (rows affected) smaller than `payload.ids.len()` whenever the request contains ids that are already-archived, nonexistent, or duplicated. The handler treats *any* such shortfall as a hard conflict:

```rust
match result {
    Ok(count) => {
        if count as usize != payload.ids.len() {
            return api_error_response(StatusCode::CONFLICT, "BATCH_ACTION_CONFLICT",
                "Some jobs could not be modified because they are active, archived, or do not exist.");
        }
        if payload.action == "delete" {
            purge_resume_sessions_for_jobs(state.as_ref(), &payload.ids).await;
        }
        // ...
```

But the `UPDATE … WHERE …` already committed — the eligible rows are archived (delete) or requeued (restart). The early `return` then:

1. **Leaks resume sessions** — for delete, `purge_resume_sessions_for_jobs` never runs, so the resume-session DB rows and on-disk temp dirs for the jobs that *were* archived are orphaned (defeats the RG-1/RG-6 cleanup intent).
2. **Silently requeues** — for restart, the eligible jobs are now `status='queued', progress=0, attempt_count=0` and will be picked up by the processor, even though the client received a 409 implying nothing happened.
3. **False conflict on duplicates** — `ids = [5, 5]` archives one row (`count = 1`), `1 != 2` → 409, although the job was deleted.

Realistic trigger: the user multi-selects from a stale jobs table where retention cleanup or another tab already archived some rows, clicks *Delete* → 409 plus orphaned resume artifacts for the rows that were deleted.

**Fix:**

1. De-duplicate the ids before comparing counts (and before the DB call) so duplicates can't manufacture a false conflict:
   ```rust
   let mut ids = payload.ids.clone();
   ids.sort_unstable();
   ids.dedup();
   ```
2. Always purge resume sessions for the rows that were actually deleted, *before* deciding whether to report a partial result — move the purge above the conflict check on the delete path:
   ```rust
   Ok(count) => {
       if payload.action == "delete" {
           purge_resume_sessions_for_jobs(state.as_ref(), &ids).await;
       }
       if count as usize != ids.len() {
           return api_error_response(StatusCode::CONFLICT, "BATCH_ACTION_CONFLICT", /* ... */);
       }
       axum::Json(serde_json::json!({ "count": count })).into_response()
   }
   ```
   (`purge_resume_sessions_for_jobs` already no-ops for ids without a session, so purging the full deduped set is safe.)
3. Prefer reporting partial success over a flat 409: return `200` with `{ "count": count, "requested": ids.len() }` so the UI can show "3 of 5 modified" instead of treating a partial apply as total failure. If all-or-nothing UX is required, do an eligibility pre-check (`get_jobs_by_ids` + filter on `is_active()`/`archived`) *before* mutating and reject atomically without touching the DB.
4. Add a regression test in `src/server/tests.rs`: enqueue two jobs, archive one, POST `/api/jobs/batch` `{action:"delete", ids:[a, b]}`, and assert the resume session for the successfully-deleted job is purged regardless of the 409/partial response.

---

### [P2-35] `/api/system/selftest` spawns an ffmpeg encode and a full migration run per request with no single-flight or cap

**Status: RESOLVED** — `system_selftest_handler` now takes `State<Arc<AppState>>` and single-flights on a new `selftest_in_progress: Arc<AtomicBool>`, returning `429 SELFTEST_BUSY` when one is already running; an RAII guard clears the flag on drop so it resets even on panic. The self-test pipeline was extracted to `src/system/selftest.rs` and is also exposed as the `alchemist selftest` CLI subcommand. (The optional single-shared-in-memory-DB optimisation from fix step 3 was left out as a non-correctness change now that the endpoint is single-flighted.)

**Files:**
- `src/server/system.rs:1237–1241` — `system_selftest_handler` calls `run_selftest().await` directly; takes no `State`, has no in-progress guard.
- `src/system/selftest.rs:35–268` — `run_selftest` runs ffprobe + a real ffmpeg encode (Execute stage) and builds a throwaway `Db::new(":memory:")` (full migration suite) on every call.
- `src/server/mod.rs:659`, `:891` — route registered at `/api/system/selftest` and `/api/v1/system/selftest` (auth-gated, but only the global 120-token bucket limits it).

**Severity:** P2

**Problem:**

The handler does all of its work synchronously inside the request, and each invocation spawns a real ffmpeg child process (the Execute stage) plus runs the entire SQLite migration suite against a fresh in-memory database:

```rust
pub(crate) async fn system_selftest_handler() -> impl IntoResponse {
    let response = crate::system::selftest::run_selftest().await;
    axum::Json(response)
}
```

There is no single-flight guard and no per-route rate cap. The codebase already established the single-flight pattern for exactly this class of endpoint — P2-24 (health scan), P2-26 (update install), and P2-30 (library preview) all gate expensive work behind an `AtomicBool`/`Semaphore` and return `409`/`429` when busy. The self-test endpoint reintroduces the un-gated pattern: a retry storm or a malicious authed client can stack concurrent ffmpeg encodes that contend with the real transcode pipeline, and re-runs every migration per call (wasted CPU/IO). "Spawning subprocesses in response to HTTP requests without rate limiting" is the same defect class as P2-30.

**Fix:**

1. Add `selftest_in_progress: Arc<AtomicBool>` to `AppState` (mirror `library_health_scan_in_progress`) and gate the handler with `compare_exchange`, returning `429 SELFTEST_BUSY` when already running. Clear the flag via an RAII guard so it resets even on panic.
2. Change `system_selftest_handler` to take `State<Arc<AppState>>` (it currently takes no arguments) so it can consult the flag.
3. Avoid the per-call full migration run: build the in-memory self-test DB once (e.g. a `OnceCell`/`tokio::sync::OnceCell`) and reuse it — safe once the endpoint is single-flighted, since the fake `Job { id: 1, .. }` insert can no longer collide across concurrent calls. Do **not** reuse `state.db` for the Execute stage; it would write a fake job row into the production database.
4. Add an integration test that fires two concurrent `POST /api/system/selftest` calls and asserts the second returns 429.

---

### [P2-36] Native mac app: bundled daemon is orphaned on app quit and never monitored for crashes

**Status: RESOLVED** — an `NSApplicationDelegateAdaptor`'s `applicationShouldTerminate`
calls `stopBundledDaemon(waitForExit:)` (SIGTERM, bounded wait, SIGKILL fallback) so the
daemon isn't orphaned. `process.terminationHandler` reports crashes (`Stopped (exit N)`
+ log tail). A pre-spawn `/api/ready` probe adopts an already-running daemon instead of
spawning a doomed duplicate. Version-strict adoption is deferred (no auth-free version
endpoint); the private port 41737 carries the anti-collision weight.

**Files:**
- `native/mac/Sources/AlchemistMac/AlchemistMac.swift:4–127` — no app-termination hook; `stopBundledDaemon()` has zero callers
- `native/mac/Sources/AlchemistMacCore/Daemon/DaemonController.swift:39–68` — no `terminationHandler`; status string set optimistically

**Severity:** P2

**Problem:**

Nothing calls `stopBundledDaemon()` — quitting the app leaves `alchemistd` (and any in-flight FFmpeg children) running forever. On next launch `process` is nil, so a second daemon is spawned; it fails to bind :3000 and dies, while `status` still reports "Running on 127.0.0.1:3000" and the app silently talks to the stale orphan — which after an app update is an older daemon version. There is also no `terminationHandler`, so a crashed daemon leaves status frozen at "Running".

**Fix:**

1. Add an `NSApplicationDelegateAdaptor` in `AlchemistMac.swift` whose `applicationShouldTerminate` calls `daemon.stopBundledDaemon()` and waits briefly (`waitUntilExit` with timeout) before allowing termination. `Process.terminate()` sends SIGTERM, which the daemon already handles cleanly.
2. Set `process.terminationHandler` in `startBundledDaemon` to hop to MainActor, set `status = "Stopped (exit \(code))"` / `lastError`, and surface `AlchemistUIError.daemonFailed`.
3. On startup, before spawning, probe `GET /api/v1/system/info`; if something is already serving, compare `version` — adopt it if it matches the staged daemon version, otherwise report the conflict instead of spawning a doomed duplicate.

---

### [P2-37] Native mac app: cold-launch race deletes the keychain session token and can skip the setup wizard

**Status: RESOLVED** — a single `AppModel.bootstrap()` (readiness poll →
setup-status → session restore → refresh) replaces the 700 ms guess.
`restoreSessionIfAvailable` deletes the keychain token only on a real
`AlchemistAPIError.unauthorized`; transient connection failures keep it.
`refreshSetupStatus` distinguishes "daemon not ready" from "setup not required" instead
of defaulting a fresh install to login. (Shared bootstrap routine is [TD-13].)

**Files:**
- `native/mac/Sources/AlchemistMacCore/AppModel.swift:37–49` — `init` starts the daemon then immediately probes, no readiness wait
- `native/mac/Sources/AlchemistMacCore/AppModel.swift:266–283` — `restoreSessionIfAvailable` deletes the token in a blanket `catch`
- `native/mac/Sources/AlchemistMacCore/AppModel.swift:165–179` — `startBundledDaemon` papers over the race with a 700 ms sleep
- `native/mac/Sources/AlchemistMacCore/AppModel.swift:235–250` — `refreshSetupStatus` swallows all errors

**Severity:** P2

**Problem:**

`AppModel.init` spawns the daemon and immediately (no delay on this path) calls `refreshSetupStatus` → error swallowed (`setupRequired` stays false — a fresh install lands on Login instead of the wizard) → `restoreSessionIfAvailable` → `fetchEngineStatus` gets connection-refused because the daemon hasn't bound yet → the `catch` treats any error as auth failure: `clearSessionToken()` + `KeychainHelper.deleteSessionToken()`. The persisted session is destroyed on effectively every cold launch; users must re-login each start. The public `startBundledDaemon()` path "fixes" the same race with `Task.sleep(700ms)` — a guess that loses on slow disks or first-run migrations.

**Fix:**

1. In `restoreSessionIfAvailable`, only clear/delete the token when the error is `AlchemistAPIError.unauthorized`. On any other error keep the token and set `connection.lastError = .connectionFailed(...)`.
2. Replace the 700 ms sleep with a readiness poll: retry `fetchSetupStatus()` with short backoff for up to ~15 s before declaring the daemon unreachable; share one bootstrap routine across `init`, `startBundledDaemon`, and `reconnect` (see TD-13).
3. In `refreshSetupStatus`, distinguish "daemon not ready" from "setup not required" instead of silently defaulting to the login flow.

---

### [P2-38] Native mac app: SSE reconnect backoff is dead code; connection state machine lies and loops forever on 401

**Status: RESOLVED** — `startEventStream` no longer flips to `.connected`/resets the
attempt counter at the top of the loop; it reports `.connecting`/`.reconnecting(attempt:)`
before each request and flips to `.connected` only on the synthetic `.connected` event
(yielded by `streamEvents` on a real 2xx). On `.unauthorized` it sets
`lastError = .authenticationRequired`, calls `stopAll()`, and returns instead of
hammering every ~2 s. Backoff now actually grows.

**Files:**
- `native/mac/Sources/AlchemistMacCore/State/ConnectionState.swift:47–79` — `startEventStream` loop

**Severity:** P2

**Problem:**

The top of every `while` iteration runs `self.sseState = .connected; self.reconnectAttempt = 0` before the request is even made. Consequences: (a) the exponential backoff `1 << min(attempt, 5)` can never exceed attempt 1 — the app retries every ~2 s forever and the banner always says "attempt 1"; (b) `sseState` claims connected while connecting or while the server is down (`.connecting` is unreachable after the first iteration); (c) on `.unauthorized` it sets `lastError` but keeps hammering the dead session every 2 s indefinitely instead of stopping and presenting login.

**Fix:**

1. Set `sseState = .connecting` before the request; flip to `.connected` and reset `reconnectAttempt` only after the stream is established (HTTP 200 / first event) — e.g. yield a synthetic connected marker from `streamEvents` or pass an `onConnected` callback.
2. On `.unauthorized`: `stopAll()`, set `lastError = .authenticationRequired`, and return — RootView's `onChange(of: connection.lastError)` already presents login.
3. Keep the increment/sleep as-is; the backoff works once the premature reset is removed.

---

### [P2-39] Native mac app: jobs tab, sort, and page changes never refetch — the table silently shows the previous filter's rows

**Status: RESOLVED** — `JobsWorkspaceView` adds `.onChange` refetches for `activeTab`,
`sortField`, `sortDescending`, and `page` (search keeps its 350 ms debounce). Page size
is centralized in `JobState.pageSize` (used by both `canGoToNextPage` and the
`fetchJobs` default), so it can't drift.

**Files:**
- `native/mac/Sources/AlchemistMacCore/State/JobState.swift:80–105` — `setTab` / `setSortField` / `toggleSortDirection` / `movePage` mutate state only
- `native/mac/Sources/AlchemistMacCore/Views/Jobs/JobsWorkspaceView.swift:47–65` — refresh wired only to `.task`, the search debounce, and the manual button
- `native/mac/Sources/AlchemistMacCore/Views/Jobs/JobsComponents.swift` — `JobTabButton` calls `setTab` only

**Severity:** P2

**Problem:**

Clicking the "Failed" tab, changing sort, or paging updates `activeTab`/`sortField`/`page` but nothing calls `refresh`. The fetched rows only change on the next refresh trigger — which, with SSE dead (P1-12), is the manual Refresh button. To the user, tabs/sort/pagination are broken; worse, the next unrelated refresh suddenly applies the pending filter, which reads as data loss.

**Fix:**

1. In `JobsWorkspaceView`, add `.onChange(of: model.jobs.activeTab)`, `.onChange(of: model.jobs.sortField)`, `.onChange(of: model.jobs.sortDescending)`, and `.onChange(of: model.jobs.page)` → `Task { await model.jobs.refresh(apiClient: model.connection.apiClient) }`. Keep the 350 ms debounce for search only.
2. While here: `canGoToNextPage` (`jobs.count == 50`) hardcodes the page size — extract `static let pageSize = 50` and use it in both the `fetchJobs(limit:)` default and the computed property.

---

### [P2-40] Native mac app: hardcoded port 3000 — bind failure is undetectable and the app trusts whatever is already listening

**Status: RESOLVED** — bundled mode binds the private port 41737
(`DaemonController.bundledPort`/`bundledBaseURLString`, consumed by `ConnectionState`).
After spawn, a `/api/ready` readiness poll confirms the daemon actually bound before
status reads Running; otherwise status flips to Unavailable with the daemon log tail.
Check `bundledModeUsesPrivatePort` locks the port. Version-strict matching on adopt is
deferred (no auth-free version endpoint).

**Files:**
- `native/mac/Sources/AlchemistMacCore/Daemon/DaemonController.swift:39` — `startBundledDaemon(port: Int = 3000)`
- `native/mac/Sources/AlchemistMacCore/State/ConnectionState.swift:14` — `baseURLString = "http://127.0.0.1:3000"`

**Severity:** P2

**Problem:**

Port 3000 is the single most common dev-server port. `try process.run()` succeeding only means the binary launched; if the bind fails the daemon exits (silently — see P1-13/P2-36) while status reports "Running on 127.0.0.1:3000", and every API call — including login with admin credentials — goes to whatever foreign process owns the port. The setup flow would even POST the new admin password there.

**Fix:**

1. Default bundled mode to an uncommon fixed port (e.g. 41737); keep 3000 only as the remote-mode suggestion.
2. After spawn, health-check `GET /api/v1/system/info` and require `version` to match the staged daemon's version before marking status Running (pairs with P2-36 step 3).
3. Surface daemon exit + stderr (P1-13) so a bind failure is visible within a second of launch.

---

### [P2-41] Setup wizard can deadlock waiting for hardware because it subscribes to an authenticated SSE stream during setup

**Status: RESOLVED** — setup no longer depends on authenticated SSE during first-run
hardware detection. `SetupWizard` polls `/api/system/hardware` directly while the
hardware state is unresolved, treats `503 HARDWARE_STATE_UNAVAILABLE` as pending, and
unlocks Review once a real payload arrives. Setup preview failures are also now surfaced
and block progression, so the completion path is deterministic. Covered by the new
`setup polls hardware until the review step can complete` and
`setup surfaces preview failures inline and blocks leaving the library step` Playwright
cases.

**Files:**
- `web/src/components/SetupWizard.tsx:67–106` — initial hardware bootstrap tolerates `503 HARDWARE_STATE_UNAVAILABLE` by setting `hardware = null`, then relies on `new EventSource("/api/events")` to hear `hardware_state_changed`.
- `web/src/components/SetupWizard.tsx:209–211` — `canComplete` blocks final submission until `hardware !== null`.
- `web/src/components/setup/SetupFrame.tsx:96–104` — the final "Complete Setup" button is disabled when `!canComplete`.
- `src/server/middleware.rs:151–184` — `/api/events` is not in the setup-mode allowlist, so it still requires a session/API token after the LAN/token gate.

**Severity:** P2

**Validation:** Code inspection. Current setup E2E covers the ready-hardware path and
does not exercise the `503 HARDWARE_STATE_UNAVAILABLE` -> wait-for-update flow.

**Problem:**

On first boot, `/api/system/hardware` is allowed during setup but can legitimately return `503 HARDWARE_STATE_UNAVAILABLE` until probing finishes. The new setup flow treats that as "pending", disables completion, and waits for a `hardware_state_changed` event:

```ts
const eventSource = new EventSource("/api/events");
const canComplete = step !== 5 || hardware !== null;
```

But setup users are not authenticated yet, and `/api/events` is still protected by `auth_middleware`. On any machine where hardware probing is slower than the first page load, the Review step can become a permanent spinner with a disabled "Complete Setup" button. If `ALCHEMIST_SETUP_TOKEN` is set, the SSE URL also omits `?token=...`, so token-gated setup deadlocks the same way.

**Fix:**

1. Do not depend on authenticated SSE for first-run setup. Preferred fix: replace the setup-time `EventSource("/api/events")` with bounded polling of `/api/system/hardware` every few hundred milliseconds while `hardware === null`, and stop polling once a real payload arrives.
2. If you keep SSE, explicitly allow `/api/events` during setup in `auth_middleware` and append the setup token query parameter when `ALCHEMIST_SETUP_TOKEN` mode is active.
3. Add an end-to-end test that makes the first `/api/system/hardware` call return `503`, later returns a valid payload, and asserts the Review step eventually enables "Complete Setup" without a page reload.

---

### [P2-42] Convert tool uploads are aborted after 30 s by the shared `apiFetch` timeout — any realistically sized file fails mid-transfer

**Status: RESOLVED (2026-06-22).** `apiFetch` takes a per-call `timeoutMs` (default 30 s,
`null` disables the abort timer). `ConversionTool` now uploads via a dedicated XHR
(`uploadConversionFile`) with no client timeout and a real upload-progress bar, and
distinguishes timeout/abort from other failures.

**Files:**
- `web/src/lib/api.ts:101–104` — `apiFetch` unconditionally arms `controller.abort()` after `timeoutMs = 30000`.
- `web/src/lib/api.ts:134–139` — the timeout fires regardless of how the call is used; there is no per-call opt-out.
- `web/src/components/ConversionTool.tsx:253–286` — `uploadFile` POSTs a `FormData` video body through `apiFetch` with no signal/override.

**Severity:** P2

**Problem:**

The Convert feature uploads a whole source video to `/api/conversion/uploads`. It goes through `apiFetch`, which aborts every request after a hardcoded 30 seconds:

```ts
const timeoutMs = 30000; // 30s timeout: hardware detection and large scans can take time
const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
```

The abort kills the entire request, including the in-flight upload body. A 30 s ceiling covers only a small file on a fast link — e.g. ~2 GB at 50 MB/s already needs ~40 s, and any multi-GB file over a normal home uplink needs minutes. So uploading a typical movie aborts partway through and surfaces as a generic "Upload failed" (the `catch` falls back to `"Upload failed"` because an `AbortError` has no useful `.message`). The backend already streams the upload to disk (P2-13 resolved), so the only thing stopping large uploads is this client timeout. Convert is effectively unusable for real media.

**Fix:**

1. Give `apiFetch` a per-call timeout override instead of a fixed 30 s. Add an option (e.g. `apiFetchOptions.timeoutMs?: number | null`) and skip arming the timer when it is `null`:
   ```ts
   export async function apiFetch(url: string, options: RequestInit & { timeoutMs?: number | null } = {}) {
       const timeoutMs = options.timeoutMs === undefined ? 30000 : options.timeoutMs;
       const timeoutId = timeoutMs == null ? null : setTimeout(() => controller.abort(), timeoutMs);
       // …clearTimeout(timeoutId) guarded by `if (timeoutId !== null)`
   }
   ```
2. In `ConversionTool.uploadFile`, pass `{ timeoutMs: null }` (rely on the browser/server for upload duration) or a generous bound, and prefer `XMLHttpRequest`/`fetch` with an upload-progress indicator so the user sees movement instead of a frozen "Uploading…".
3. Distinguish abort/timeout from other failures in the `catch` so the user sees "Upload timed out" rather than a bare "Upload failed".
4. Add a client unit/e2e test that a request whose body outlasts the default window is not aborted when `timeoutMs: null` is passed.

---

## Technical Debt

---

### [TD-1] `db.rs` is a 3481-line monolith

**Status: RESOLVED**

---

### [TD-2] `AlchemistEvent` legacy bridge is dead weight

**Status: RESOLVED**

---

### [TD-3] `pipeline.rs` legacy `AlchemistEvent::Progress` stub

**Status: RESOLVED**

---

### [TD-4] Silent `.ok()` on pipeline decision and attempt DB writes

**Status: RESOLVED**

---

### [TD-5] Correlated subquery for sort-by-size in job listing

**Status: RESOLVED**

---

### [TD-6] Blocking synchronous calls in async handlers

**Status: RESOLVED**

---

### [TD-7] Blocking Argon2 hashing in setup wizard

**Status: RESOLVED**

---

### [TD-8] Blocking Argon2 verification in auth

**Status: RESOLVED**

---

### [TD-9] Blocking `std::fs::canonicalize` in enqueue hot path

**Status: RESOLVED**

**Files:**
- `src/server/jobs.rs:161–169` — async handler loops over allowed roots calling synchronous `std::fs::canonicalize` per entry.

**Severity:** TD

**Problem:**

`enqueue_job_from_submitted_path` is an async handler. Line 116 correctly uses `tokio::fs::canonicalize(&requested_path).await`. Line 163 does the opposite for every allowed root:

```rust
for root in allowed_roots {
    if let Ok(canonical_root) = std::fs::canonicalize(&root) {
```

Each iteration blocks the async worker on a `stat`/`readlink` chain. With a dozen watch folders on a slow NFS mount, the worker is parked for tens of milliseconds, starving other handlers. The function is reached from both `enqueue_job_handler` and the ARR webhook ingress, so a misconfigured Sonarr can amplify the cost.

**Fix:**

1. Replace the loop body with `tokio::fs::canonicalize(&root).await`:
   ```rust
   for root in allowed_roots {
       if let Ok(canonical_root) = tokio::fs::canonicalize(&root).await {
           if canonical_path.starts_with(&canonical_root) {
               is_allowed = true;
               break;
           }
       }
   }
   ```
2. Optionally cache canonicalised allowed roots on `AppState` and invalidate on watch-dir / config change. Larger refactor; not required for the TD fix.

---

### [TD-10] `update::stage_update_asset` blocks the runtime on tar + version probe

**Status: RESOLVED**

**Files:**
- `src/update.rs:234–267` — `stage_update_asset` is async but calls blocking `extract_archive` and `verify_staged_binary_version`.
- `src/update.rs:578–620` — both helpers use `std::process::Command::new(...).output()` directly.

**Severity:** TD

**Problem:**

`stage_update_asset` is `async fn` and is called from the `install_system_update_handler` async path. After `tokio::fs::write(&archive_path, bytes).await?`, it calls:

```rust
let staged_binary_path = extract_archive(&archive_path, &staging_dir)?;
verify_staged_binary_version(&staged_binary_path, version)?;
```

Both functions use synchronous `std::process::Command::output()` which blocks the calling worker for the entire `tar -xzf` plus `alchemist --version` runtime. On large releases (web assets baked in, ~80–120 MB), this can be multiple seconds — long enough to starve other tokio tasks on the same worker, including the SSE event loop.

**Fix:**

1. Wrap both helpers in `tokio::task::spawn_blocking`:
   ```rust
   let staged_binary_path = tokio::task::spawn_blocking({
       let archive = archive_path.clone();
       let dir = staging_dir.clone();
       move || extract_archive(&archive, &dir)
   })
   .await
   .map_err(|err| anyhow!("extract worker failed: {err}"))??;
   ```
2. Same treatment for `verify_staged_binary_version`. Keeps the helpers unit-testable from sync contexts and confines the blocking work to the blocking pool.

---

### [TD-11] `device_id_for` blocks the async runtime on every enqueue

**Status: RESOLVED** — added `device_id_for_async` (wraps the sync resolver in `spawn_blocking`); `enqueue_job` now awaits it. The sync `device_id_for` is retained for the blocking-pool closure and unit tests.

**Files:**
- `src/system/device_id.rs:13–23` — `device_id_for` calls `std::fs::canonicalize` and `std::fs::metadata` (blocking syscalls).
- `src/db/jobs.rs:29–75` — `enqueue_job` is `async fn` and invokes `device_id_for(input_path)` synchronously on line 53.
- `src/media/pipeline.rs:1043–1072` — `enqueue_discovered_with_db` is the scanner's hot path; it calls `enqueue_job` once per discovered file.

**Severity:** TD

**Problem:**

PERF-2 introduced per-job source-device resolution. The resolver is a synchronous helper that does two blocking syscalls (canonicalize + metadata) and is invoked from an async path:

```rust
// src/db/jobs.rs
pub async fn enqueue_job(...) -> Result<bool> {
    ...
    let source_device = crate::system::device_id::device_id_for(input_path);
    // ↑ std::fs::canonicalize + std::fs::metadata, both blocking
    let result = sqlx::query("INSERT INTO jobs ...").bind(...).await?;
```

Library scans enqueue files in a tight loop (`for file in all_scanned { enqueue_discovered_with_db(&db, file).await }` in `src/system/scanner.rs`), so a 5000-file scan parks the tokio worker on 10000 blocking syscalls back-to-back. ARR webhook ingress hits the same path. On fast local FS this is microseconds and survives; on a slow NFS mount the worker can be parked for tens of seconds, starving the SSE event loop and other handlers.

The same pattern was the basis for TD-9 (`std::fs::canonicalize` in enqueue path validation, resolved 2026-05-12); the PERF-2 addition reintroduced it on a new line.

**Fix:**

1. Push the resolution onto the blocking pool:
   ```rust
   // src/system/device_id.rs
   pub async fn device_id_for_async(path: &Path) -> Option<String> {
       let path = path.to_path_buf();
       tokio::task::spawn_blocking(move || device_id_for(&path))
           .await
           .ok()
           .flatten()
   }
   ```
2. Update `enqueue_job` to `.await` the async variant:
   ```rust
   let source_device = crate::system::device_id::device_id_for_async(input_path).await;
   ```
3. Keep the sync `device_id_for` exported for unit tests and any non-async callers; mark it `#[cfg(test)]`-only if there are no remaining sync callers in production.
4. Optionally cache resolved devices per watch-root canonical path to avoid repeating the canonicalize for every file under the same root. Out of scope for the TD fix; track separately if scan throughput is a real concern.

---

### [TD-12] Dead `_unused_ensure_public_endpoint` function in notifications.rs

**Status: RESOLVED** — the function was deleted; `build_safe_client` is the sole live SSRF guard.

**Files:**
- `src/notifications.rs:1055–1090` — `_unused_ensure_public_endpoint`, an `async fn` prefixed with `_unused_` and never called.

**Severity:** TD

**Problem:**

`_unused_ensure_public_endpoint` is ~36 lines of dead code carried since the SSRF work. The underscore prefix silences the dead-code lint instead of removing the function. Its logic is superseded by `build_safe_client` + `is_private_ip`. Dead code like this rots: a future reader may mistake it for the live SSRF guard and "fix" the wrong place (and indeed it shares the IPv4-mapped blind spot of P2-32).

**Fix:**

1. Delete `_unused_ensure_public_endpoint` outright. `build_safe_client` is the live guard.
2. Confirm nothing references it (`grep _unused_ensure_public_endpoint src/`) before removal — expected: zero hits outside the definition.
3. This is a good candidate for the next `/hygiene deadcode` pass if a broader sweep is preferred.

---

### [TD-13] Native mac app: three divergent bootstrap paths (init / startBundledDaemon / reconnect), one with a magic 700 ms sleep

**Status: RESOLVED** — `init`, `startBundledDaemon()`, and `reconnect()` all call a single
`AppModel.bootstrap()` (readiness poll → setup status → session restore → refresh). The
700 ms sleep is gone.

**Files:**
- `native/mac/Sources/AlchemistMacCore/AppModel.swift:37–49, 165–179, 223–233` — three near-identical async bootstrap sequences

**Severity:** TD

**Problem:**

`init`, `startBundledDaemon()`, and `reconnect()` each hand-roll `refreshSetupStatus → restoreSessionIfAvailable → refreshAll` with subtle differences (only one sleeps 700 ms; init also requests notification permission). The divergence already produced P2-37; every future startup fix must be applied three times.

**Fix:**

1. Extract a single `private func bootstrap() async` containing the readiness poll (P2-37 step 2) + setup check + session restore + refresh; call it from all three entry points.

---

## Reliability Gaps

---

### [RG-1] No encode resume after crash or restart

**Status: RESOLVED**

---

### [RG-2] AMD VAAPI/AMF hardware paths unvalidated

**Status: RESOLVED** (Added unit test coverage for command generation)

---

### [RG-3] Daily summary scheduling can miss a day, suppress retries, and duplicate after restart

**Status: RESOLVED**

---

### [RG-4] Library health scans still probe archived jobs

**Status: RESOLVED**

---

### [RG-5] Library health scan endpoint allows overlapping full-library runs

**Status: RESOLVED**
**Status: REGRESSED — see P2-24** (atomic flag plumbed but never read by the handler)
**Status: RESOLVED — re-fixed via P2-24** (handler now gates on the atomic with compare_exchange)

---

### [RG-6] Cancelled backup downloads leave full SQLite snapshots behind in the temp directory

**Status: RESOLVED**

---

### [RG-7] Performance bottleneck: Non-indexed date filters in stats queries

**Status: RESOLVED**

---

### [RG-8] Silent unwrap_or_default() on serde_json in MCP server

**Status: RESOLVED**

---

### [RG-9] Update archive is buffered entirely into memory before disk write

**Status: RESOLVED**

**Files:**
- `src/update.rs:241–256` — `stage_update_asset` calls `.bytes().await?` on the reqwest response and only then writes to disk.
- `src/update.rs:571–576` — `create_update_staging_dir` creates the directory but has no cleanup path on failure.

**Severity:** RG

**Problem:**

```rust
let bytes = client.get(&asset.url).send().await?.error_for_status()?.bytes().await?;
```

The full update archive lives in RAM before any verification or filesystem write. Today's payloads are 80–120 MB; nothing prevents future releases from being larger. On low-memory containers (Docker on a Pi 4, 1 GB RAM) the buffered download can OOM the process. Combined with the lack of staging cleanup on failure, each failed retry leaves another archive's worth of data in `temp_dir/update-*/`.

**Fix:**

1. Stream the download to disk, hashing on the fly:
   ```rust
   let mut response = client.get(&asset.url).send().await?.error_for_status()?;
   let mut file = tokio::fs::File::create(&archive_path).await?;
   let mut hasher = Sha256::new();
   while let Some(chunk) = response.chunk().await? {
       hasher.update(&chunk);
       tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
   }
   let actual_sha = format!("{:x}", hasher.finalize());
   if !actual_sha.eq_ignore_ascii_case(&asset.sha256) {
       tokio::fs::remove_file(&archive_path).await.ok();
       return Err(anyhow!("downloaded asset hash mismatch ..."));
   }
   ```
2. Wrap the staging dir in an RAII cleanup struct (mirror `SnapshotCleanup` in `src/server/system.rs:424–441`) so any error path between `create_update_staging_dir` and `Ok(StagedUpdate)` removes the directory.
3. Add a streaming test that fakes a 50 MB body via the same local listener pattern used in `notifications.rs` tests and asserts the staged file matches the source byte-for-byte and SHA-256.

---

### [RG-10] IPv4-Mapped IPv6 Addresses Bypass/Lockout in LAN and Trusted Proxy Checks

**Status: RESOLVED in 0.3.4-rc.2.** IPv4-mapped IPv6 addresses are normalized
before LAN classification, trusted-proxy comparison, forwarded-client
resolution, and rate-limit keying.

**Files:**
- `src/server/middleware.rs:468–473` — `is_lan_ip` matches on IPv4/IPv6 branches directly.
- `src/server/middleware.rs:449–466` — `is_trusted_peer` checks loopback and private lists directly.

**Severity:** RG

**Problem:**

On dual-stack systems connected to local or trusted clients, peer/proxy IPs are represented as IPv4-mapped IPv6 addresses (`::ffff:a.b.c.d`). In `is_lan_ip` and `is_trusted_peer`, matching branches for `IpAddr::V6` only look for loopback (`::1`), link-local, or unique local addresses and do not normalize mapped addresses. Consequently, connections from `::ffff:127.0.0.1` or `::ffff:192.168.1.1` fail to be recognized as LAN/trusted, locking operators out of the setup wizard or metrics scrapers under dual-stack configurations.

```rust
fn is_lan_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}
```

**Fix:**

1. In `src/server/middleware.rs`, update `is_lan_ip` to normalize IPv4-mapped IPv6 addresses to IPv4 using `to_ipv4_mapped()` before doing the classifications:
   ```rust
   fn is_lan_ip(ip: IpAddr) -> bool {
       match ip {
           IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
           IpAddr::V6(v6) => {
               if let Some(v4) = v6.to_ipv4_mapped() {
                   return is_lan_ip(IpAddr::V4(v4));
               }
               v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local()
           }
       }
   }
   ```
2. Mirror the same normalization in `is_trusted_peer`, normalizing both the checked `ip` and the configured `trusted_proxies` so mapped and standard forms compare cleanly:
   ```rust
   fn is_trusted_peer(ip: IpAddr, trusted_proxies: &[IpAddr]) -> bool {
       let normalized_ip = match ip {
           IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
           _ => ip,
       };

       let is_loopback = match normalized_ip {
           IpAddr::V4(v4) => v4.is_loopback(),
           IpAddr::V6(v6) => v6.is_loopback(),
       };
       if is_loopback {
           return true;
       }

       if trusted_proxies.is_empty() {
           match normalized_ip {
               IpAddr::V4(v4) => v4.is_private() || v4.is_link_local(),
               IpAddr::V6(v6) => v6.is_unique_local() || v6.is_unicast_link_local(),
           }
       } else {
           trusted_proxies.iter().any(|&proxy| {
               let normalized_proxy = match proxy {
                   IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(proxy),
                   _ => proxy,
               };
               normalized_ip == normalized_proxy
           })
       }
   }
   ```
3. Add a unit test verifying `::ffff:127.0.0.1` and other mapped IPs are correctly classified in loopback/LAN/trusted functions.

---

### [RG-11] Self-test 'Write Temp' failure leaves the temp directory behind

**Status: RESOLVED** — the Write-Temp error path now calls `cleanup_temp_dir(&temp_dir)` before returning (`src/system/selftest.rs:72`), matching every other stage's failure path.

**Files:**
- `src/system/selftest.rs:46–78` — the 'Write Temp' stage `create_dir_all`s the temp dir, then on a write failure returns early without calling `cleanup_temp_dir`.

**Severity:** RG

**Problem:**

`run_selftest` creates `temp_dir` via `tokio::fs::create_dir_all(&temp_dir)` and then writes the fixture. If `create_dir_all` succeeds but `tokio::fs::write(&input_path, …)` fails (disk full, permissions, interrupted), the function returns the failure response immediately:

```rust
Err(e) => {
    stages.push(/* Write Temp failed */);
    return SelftestResponse { success: false, stages, error: Some(/* ... */) };
    // ← no cleanup_temp_dir(&temp_dir) call
}
```

Every other stage's failure path (Analyze/Plan/Execute) calls `cleanup_temp_dir` before returning; only the Write-Temp path skips it, orphaning the freshly created `alchemist-selftest-<uuid>` directory in the system temp dir. Each failed self-test under a persistently-failing condition leaves another empty dir behind. Mirrors the leftover-temp-artifact class already tracked in RG-6/RG-9.

**Fix:**

1. Call `cleanup_temp_dir(&temp_dir)` on the Write-Temp failure path before returning (it already no-ops when the path doesn't exist):
   ```rust
   Err(e) => {
       stages.push(/* ... */);
       let _ = cleanup_temp_dir(&temp_dir).await;
       return SelftestResponse { success: false, stages, error: Some(/* ... */) };
   }
   ```
2. Alternatively, refactor the pipeline body into a `Result`-returning inner async block and run a single trailing `cleanup_temp_dir` on every exit path. The single-cleanup form removes the per-arm duplication that caused the omission in the first place.

---

### [RG-12] Native mac app: shared URLSession cookie jar fights the manually managed session token

**Status: RESOLVED** — `AlchemistAPIClient` now builds a dedicated ephemeral session
(`httpShouldSetCookies = false`, `httpCookieAcceptPolicy = .never`) by default instead of
`URLSession.shared`; the injectable `session:` parameter is retained. Login still reads
`Set-Cookie` off the response.

**Files:**
- `native/mac/Sources/AlchemistMacCore/API/AlchemistAPIClient.swift:94–98` — uses `URLSession.shared`
- `native/mac/Sources/AlchemistMacCore/API/AlchemistAPIClient.swift:460–464` — sets the `Cookie` header manually

**Severity:** RG

**Problem:**

`URLSession.shared` has `httpShouldSetCookies = true` and a persistent cookie jar, so the backend's `Set-Cookie: alchemist_session=...` is stored and auto-attached by the session — and per URLSession documentation, cookie storage can override a manually set `Cookie` header. After logout/`clearSessionToken` or login as a different user, the jar can still inject the old cookie; which credential wins (jar cookie vs. manual cookie vs. `Bearer` header) is undefined from the app's perspective. Works today mostly by luck; violates the deterministic-behavior design rule.

**Fix:**

1. In `init`, build a dedicated session: `let config = URLSessionConfiguration.ephemeral; config.httpShouldSetCookies = false; config.httpCookieAcceptPolicy = .never; self.session = URLSession(configuration: config)` (keep the injectable `session:` parameter for checks).
2. Cookie extraction from the login response keeps working — `Set-Cookie` is still present on the response even when not stored.

---

### [RG-13] Native mac app: status-change handler for the focused job is an empty placeholder — open inspector goes stale

**Status: RESOLVED** — the empty `if` block in `JobState.handleStatusChange` is gone;
`AppModel.handleEvent`'s `.status` case now reloads `focusedDetail` via
`tasks.run("job-detail-refresh")` when the changed job is the open one, mirroring the
`.decision`/`.log` path.

**Files:**
- `native/mac/Sources/AlchemistMacCore/State/JobState.swift:368–375` — `handleStatusChange` contains `if focusedDetail?.job.id == jobID { … }` with an empty body
- `native/mac/Sources/AlchemistMacCore/AppModel.swift:88–94` — the `.status` event path never reloads `focusedDetail`

**Severity:** RG

**Problem:**

When the job open in the inspector changes status (e.g. encoding → completed), the table row updates but `focusedDetail` keeps the stale status, encode stats, and logs; only a later `.decision`/`.log` event happens to trigger a detail reload. The dead `if` block documents the intent and does nothing.

**Fix:**

1. In `AppModel.handleEvent`'s `.status` case, mirror the `.decision`/`.log` path: if `jobs.focusedDetail?.job.id == jobID`, `tasks.run("job-detail-refresh") { await self.jobs.loadDetails(...) }`.
2. Delete the empty `if` block in `handleStatusChange` (keep the row update).

---

### [RG-14] Windows release smoke can fail after all assertions pass because `server.log` is still locked during temp-dir cleanup

**Status: RESOLVED** — `release_smoke.py` now uses an explicit temp-root lifecycle
(`mkdtemp` + `try/finally`) instead of `TemporaryDirectory` cleanup, writes logs outside
the auto-deleted temp root, and retries cleanup on Windows `PermissionError`s before
downgrading cleanup failure to a warning. Local validation:
`python3 scripts/release_smoke.py --binary ./target/debug/alchemist --expected-version 0.3.4-rc.2`
passed end to end on 2026-06-13.

**Files:**
- `scripts/release_smoke.py:75–83` — `stop()` terminates the server and waits, but the harness has no Windows-specific retry/release logic for the inherited log handle.
- `scripts/release_smoke.py:101–142` — the smoke runs two server launches inside one `TemporaryDirectory`, reuses `server.log`, and relies on context-manager cleanup to succeed after the final stop.

**Severity:** RG

**Validation:** Remote release evidence. Confirmed in GitHub Release run `27072707398`,
job `79905069761` (`smoke / native-windows`) on 2026-06-06.

**Problem:**

The latest published RC proves this path is still flaky. GitHub Actions release run `27072707398` (job `79905069761`, June 6, 2026) completed the native Windows version check, server boot, setup, and `alchemist selftest`, then failed only when Python tried to delete `server.log` from the temp directory:

```text
PermissionError: [WinError 32] The process cannot access the file because it is being used by another process: '...\\server.log'
```

That means the release gate can fail even when the artifact itself passes the smoke. The current `ignore_cleanup_errors=True` mitigation is demonstrably insufficient on the hosted Windows runner, so release promotion remains fragile.

**Fix:**

1. Stop relying on `TemporaryDirectory` cleanup for the Windows path. Create the temp root with `tempfile.mkdtemp()`, wrap the whole flow in `try/finally`, and perform explicit `shutil.rmtree(...)` cleanup after all assertions.
2. In the `finally` block, add a short retry loop on Windows (`PermissionError` with 50–250 ms backoff for a few seconds) before giving up on deletion. Cleanup failure after a successful smoke should log a warning, not fail the release.
3. Keep `server.log` outside the temp root, or rotate to a per-launch log file and close/flush it before teardown, so the temp-root delete is not coupled to Windows file-handle timing.
4. Add a regression step in CI that exercises the exact Windows smoke path after `stop(process)` and asserts the harness exits `0` even when log cleanup needs retries.

---

### [RG-15] Convert tool polls the conversion-status endpoint every 2 s forever — it never stops once the job reaches a terminal state

**Status: RESOLVED (2026-06-22).** The status-poll effect is now keyed on `status?.status`
and returns early once the state is terminal (`completed`/`failed`/`cancelled`); repeated
poll errors surface a warning toast instead of being silently swallowed.

**Files:**
- `web/src/components/ConversionTool.tsx:183–191` — the status-poll `useEffect` is keyed only on `conversionJobId`; the interval is cleared on id change/unmount but never when `status.status` becomes terminal.

**Severity:** RG

**Problem:**

```ts
useEffect(() => {
    if (!conversionJobId) return;
    const id = window.setInterval(() => {
        void apiJson<JobStatusResponse>(`/api/conversion/jobs/${conversionJobId}`)
            .then(setStatus).catch(() => undefined);
    }, 2000);
    return () => window.clearInterval(id);
}, [conversionJobId]);
```

Once a conversion job id is set, the component hits `/api/conversion/jobs/{id}` every 2 s for as long as the Convert page stays mounted — including after the job is `completed`, `failed`, or `cancelled`, when there is nothing left to learn. A user who converts a file and leaves the tab open generates an indefinite 0.5 req/s stream against the API (and each request runs a DB read). Errors are swallowed (`.catch(() => undefined)`), so a backend hiccup is invisible too. Not data loss, but needless sustained load that scales with idle Convert tabs.

**Fix:**

1. Stop polling when the status is terminal. Track terminal states and clear/skip:
   ```ts
   useEffect(() => {
       if (!conversionJobId) return;
       const terminal = new Set(["completed", "failed", "cancelled"]);
       if (status && terminal.has(status.status)) return;
       const id = window.setInterval(/* …poll… */, 2000);
       return () => window.clearInterval(id);
   }, [conversionJobId, status?.status]);
   ```
2. Optionally back the live status off the existing SSE stream instead of a fixed 2 s poll, matching how the Jobs page reconciles state.
3. Surface repeated poll failures (e.g. after N consecutive errors) instead of silently swallowing them.

---

## UX Gaps

---

### [UX-1] Queued jobs show no position or estimated wait time

**Status: RESOLVED**

---

### [UX-2] No way to add a single file to the queue via the UI

**Status: RESOLVED**

---

### [UX-3] Workers-blocked reason not surfaced for queued jobs

**Status: RESOLVED**

---

### [UX-4] Job detail modal can jump back to an older job after out-of-order fetches

**Status: RESOLVED**

---

### [UX-5] Theme first-paint script lives in `<body>`, not `<head>`, so the cached profile still flashes briefly

**Status: RESOLVED** — the `<script is:inline>` was moved into `<head>` (before `<ClientRouter />`) so the cached profile is applied before the browser's first paint.

**Files:**
- `web/src/layouts/Layout.astro:25–53` — `<script is:inline>` placed at the end of `<body>` after `<slot />`, `ToastRegion`, `AuthGuard`, `ThemeBootstrap`.

**Severity:** UX

**Problem:**

0.3.2-rc.3 added a localStorage-backed first-paint script so cross-page navigation doesn't snap back to Helios Orange. The script does the right thing, but it's placed at the *end* of `<body>`:

```astro
<body>
    <slot />
    <ToastRegion client:load />
    <AuthGuard client:load />
    <ThemeBootstrap client:load />
    <script is:inline>
        function initTheme() {
            try {
                const cached = localStorage.getItem("theme");
                document.documentElement.setAttribute("data-color-profile", cached || "helios-orange");
            } catch { ... }
        }
        initTheme();
        document.addEventListener("astro:after-swap", initTheme);
    </script>
</body>
```

The browser parses `<html>` (no `data-color-profile`) → `<head>` → starts `<body>` rendering → only **after** all the body content is parsed does the inline script run. Between those steps the page paints with the CSS default (Helios Orange). On a heavy island like `JobManager`, the visible flash lasts long enough to be the exact UX regression the fix targeted.

The `astro:after-swap` re-run does prevent a *second* flash on ClientRouter navigation, but the initial cold paint remains affected.

The standard dark-mode-flash mitigation is to place the inline script in `<head>` before any `<body>` content. Astro supports this directly.

**Fix:**

1. Move the `<script is:inline>` from the end of `<body>` to inside `<head>` (after `<title>`, before `<ClientRouter />`):
   ```astro
   <head>
       <meta charset="UTF-8" />
       <meta name="description" content="Alchemist Media Transcoder" />
       <meta name="viewport" content="width=device-width" />
       <meta name="generator" content={Astro.generator} />
       <title>{title}</title>
       <script is:inline>
           function initTheme() { /* unchanged */ }
           initTheme();
           document.addEventListener("astro:after-swap", initTheme);
       </script>
       <ClientRouter />
   </head>
   ```
2. Confirm with the Playwright `e2e/dashboard-ui.spec.ts` mock setup that the data attribute is present before the body renders — a simple `await expect(page.locator("html")).toHaveAttribute("data-color-profile", "aurora-violet")` immediately after navigation, with `localStorage.theme = "aurora-violet"` pre-set, will catch a regression.
3. Optional: pre-render `data-color-profile` server-side from a signed cookie so the very first authenticated visit also avoids the flash. Larger scope — track separately.

---

### [UX-6] Native mac app: ⌘Space "Start Queue" shortcut belongs to Spotlight — it can never fire

**Status: RESOLVED** — Start/Pause Queue are rebound to ⌘⌥S / ⌘⌥P, off the
Spotlight-owned ⌘Space / ⌘⇧Space.

**Files:**
- `native/mac/Sources/AlchemistMac/AlchemistMac.swift:91–99` — `.keyboardShortcut(.space, modifiers: [.command])` and `[.command, .shift]`

**Severity:** UX

**Problem:**

⌘Space is reserved system-wide by Spotlight (and ⌘⇧Space is commonly bound too); macOS swallows it before the app sees it. The menu items work by click only, and the displayed shortcut is a lie.

**Fix:**

1. Rebind Start/Pause Queue to free combos, e.g. `.keyboardShortcut("s", modifiers: [.command, .option])` for Start and `.keyboardShortcut("p", modifiers: [.command, .option])` for Pause.

---

### [UX-7] Native mac app: cancelled jobs notify "Encode failed"; notifications miss jobs outside the loaded page

**Status: RESOLVED** — `postStatusNotification` gives `cancelled` its own "Encode
cancelled" copy (distinct from `failed`) and falls back to "Job #\(id)" when the job
isn't in the loaded page, so notifications are no longer filter-dependent.

**Files:**
- `native/mac/Sources/AlchemistMacCore/AppModel.swift:295–305` — `postStatusNotification`

**Severity:** UX

**Problem:**

`"cancelled"` (a user action) posts the title "Encode failed" — alarming and wrong. And the `jobs.jobs.first(where:)` guard means any job not in the currently loaded 50-row page produces no notification at all, so completion notifications are filter-dependent.

**Fix:**

1. Give `cancelled` its own copy ("Encode cancelled") or skip notifying for user-initiated cancels.
2. When the job isn't in the loaded page, fetch its name (`fetchJobDetails`) or fall back to "Job #\(id)" instead of dropping the notification.

---

### [UX-8] Setup library preview failures are swallowed, so invalid folders look accepted until the final submit fails

**Status: RESOLVED** — preview failures are now kept in setup state, shown inline on the
Library step, and treated as validation failures before the wizard can leave the Library
step or complete setup. Review also shows explicit preview-state messaging instead of a
silent `--`. Covered by the same new Playwright cases that validate the setup
completion gate.

**Files:**
- `web/src/components/setup/LibraryStep.tsx:53–79` — `fetchPreview()` throws a detailed error, but the debounce effect immediately drops it with `.catch(() => undefined)`.
- `web/src/components/SetupWizard.tsx:213–220` — Review shows previewed-media count as `--`, but there is no inline explanation and the user can continue.

**Severity:** UX

**Validation:** Code inspection. Current setup E2E does not inject a failing
`/api/fs/preview` response for this path.

**Problem:**

The Library step does perform a server-side preview, but when that preview fails the user sees no feedback at all:

```ts
const handle = window.setTimeout(() => {
    void fetchPreview().catch(() => undefined);
}, 350);
```

So a mistyped or unreadable folder can sit in the selected list looking valid, the Review step just shows `Previewed media files: --`, and the first explicit error may not appear until `POST /api/setup/complete` rejects the config. This makes the setup flow feel arbitrary, especially for Docker/NAS users already struggling with path semantics.

**Fix:**

1. Keep the preview failure in component state and render it inline under the directory list, next to the existing `directoriesError`.
2. Treat "preview failed" as a step-level warning or validation failure for Next/Complete until the user fixes or removes the offending path.
3. In Review, replace the bare `--` with an explicit warning such as "Preview failed; verify server path access" so the missing preview is legible.
4. Add an e2e test where `/api/fs/preview` returns a path-access error and assert the Library step surfaces that message without waiting for final submit.

---

### [UX-9] "Save View" uses a native `window.prompt`, bypassing the app's dialog system — no validation, no styling, blockable by the browser

**Status: RESOLVED (2026-06-22).** Replaced with a themed, focus-trapped `SaveViewDialog`
(modeled on `EnqueuePathDialog`): Esc/Enter handling, inline required + duplicate-name
validation, and consistent styling. No native `window.prompt` remains.

**Files:**
- `web/src/components/JobManager.tsx:750–758` — the Save View button calls `window.prompt("View name")`.

**Severity:** UX

**Problem:**

Every other interactive flow in the app uses the in-app primitives — `ConfirmDialog`, portal modals, and toasts (the Jobs page alone wires up `ConfirmDialog`, `EnqueuePathDialog`, and a shortcuts modal). Saving a job view is the one exception: it pops a native `window.prompt`. That dialog ignores the Helios theme, can be suppressed entirely by the browser ("prevent this page from creating additional dialogs"), offers no inline validation (empty/duplicate names are only caught after the fact by a toast in `saveCurrentView`), and is jarring next to the rest of the polished UI. On the convert/jobs surface this is the only place a user meets a raw browser dialog.

**Fix:**

1. Replace `window.prompt` with a small in-app input dialog (reuse the `EnqueuePathDialog` pattern or a generic prompt modal) bound to component state, with the existing trim/duplicate validation shown inline before submit.
2. Keep the keyboard affordances the rest of the app has (Esc to cancel, Enter to confirm, focus trap) for consistency.
3. Optional: prevent saving a view whose (tab, sort, search) signature already matches an existing saved view, surfacing it inline rather than creating a duplicate.

---

## Feature Gaps

---

### [FG-4] Intelligence page content not actionable

**Status: RESOLVED**

---

### [FG-5] Duplicate intelligence misses same-title files when the container or extension differs

**Status: RESOLVED**

---

### [FG-6] Native mac app: remote connection mode is half-wired — picker does nothing, local paths sent to remote servers

**Status: RESOLVED** — remote mode is fully wired. `AppModel.setConnectionMode` stops the
bundled daemon and reconnects when switching to remote, and restarts it when switching
back. `ConnectionState.isRemote` guards `enqueueFiles`/`addWatchFolders` (clear error
instead of sending Mac-local paths), and `SettingsView`/`ConvertView` disable local-path
import + drop-to-enqueue in remote mode while keeping upload-based Convert.

**Files:**
- `native/mac/Sources/AlchemistMacCore/Views/Settings/SettingsView.swift` — Mode picker only sets `connectionMode`
- `native/mac/Sources/AlchemistMacCore/AppModel.swift:193–219` — `enqueueFiles`/`addWatchFolders` send Mac-local paths
- `native/mac/Sources/AlchemistMacCore/Views/Convert/ConvertView.swift` — drag-drop and pickers likewise

**Severity:** FG

**Problem:**

The README scopes remote mode as "later", but the UI already exposes the picker and it changes nothing (the bundled daemon keeps running; base-URL edits work regardless of mode). In remote mode, Enqueue / Watch Folder / drop send Mac-local filesystem paths to a server that can't see them — guaranteed confusing failures. Only Convert (upload) is remote-safe.

**Fix:**

1. Until remote mode is real: hide the Mode picker or mark Remote as disabled/experimental.
2. When wiring it: switching to remote should `stopBundledDaemon()` + reconnect; in remote mode disable local-path features (Enqueue Files, Watch Folder, drop-to-enqueue) and keep upload-based Convert; switching back to bundled should restart the daemon.

---

## What To Fix First

**The 2026-05-14 round (four P2s, one TD, one UX) is fully addressed.** P2-28 file_id verification, P2-29 Windows LIKE escaping, P2-30 preview single-flight + cap + path bound, P2-31 archived-job filter on reason trends, TD-11 `device_id_for` off the runtime, and UX-5 theme-flash are all resolved with regression tests; `just check-rust` and the full lib suite (247 tests) pass.

**The 2026-05-15 sweep (one P2, one TD):**

1. **[P2-32] Notification SSRF guard misses IPv4-mapped IPv6** — **RESOLVED.** `is_private_ip` now normalizes `::ffff:a.b.c.d` via `to_ipv4_mapped()` so mapped internal addresses can no longer slip past `build_safe_client`.
2. **[TD-12] Dead `_unused_ensure_public_endpoint`** — **RESOLVED.** The ~36-line dead function was deleted.

**The entire 2026-05-15 sweep is now closed.** `just check-rust` and the full lib suite (248 tests) pass.

**The 2026-05-19 sweep (one P2, one RG):**

1. **[P2-33] Rate Limiting DOS via Reverse Proxy IP in Login Handler** — **RESOLVED.** Login and global rate limiting now use the same trusted-proxy-aware resolved client IP.
2. **[RG-10] IPv4-Mapped IPv6 Addresses Bypass/Lockout in LAN and Trusted Proxy Checks** — **RESOLVED.** IPv4-mapped IPv6 addresses are normalized across client resolution, LAN checks, proxy checks, and limiter keys.

**The entire 2026-05-19 sweep is now closed in 0.3.4-rc.2.**

**The 2026-05-29 sweep (review of the system self-test branch — two P2, one RG): all three RESOLVED and shipped in 0.3.3.**

1. **[P2-34] Batch delete/restart half-applies then returns 409** — **RESOLVED.** Handler de-dupes ids and pre-checks eligibility (`get_jobs_by_ids`) before any mutation, so the batch is rejected atomically instead of half-committing; resume-session purge runs on the deduped set; DB-layer guards block active/archived rows. Test `test_batch_mutation_safety_predicates` added.
2. **[P2-35] `/api/system/selftest` un-gated subprocess + per-call migrations** — **RESOLVED.** Single-flighted on a new `selftest_in_progress` atomic (`429 SELFTEST_BUSY` when busy) with an RAII guard; pipeline extracted to `src/system/selftest.rs` and exposed as the `alchemist selftest` CLI.
3. **[RG-11] Self-test 'Write Temp' failure leaks the temp directory** — **RESOLVED.** The Write-Temp error path now calls `cleanup_temp_dir` before returning.

**Carried into 0.3.3 as known-open and resolved in 0.3.4-rc.2:**

1. **[P2-33] Rate Limiting DOS via Reverse Proxy IP in Login Handler** — **RESOLVED.** Trusted proxy forwarding now produces per-client login limiter keys without accepting spoofed headers from untrusted peers.
2. **[RG-10] IPv4-Mapped IPv6 Addresses Bypass/Lockout in LAN and Trusted Proxy Checks** — **RESOLVED.** `::ffff:a.b.c.d` is normalized before all relevant classification and keying.

*Note: the stricter `400` on unknown `/api/jobs/table?status=` tokens (introduced in the same branch) was reviewed and deliberately **not** logged — it matches the convention P1-11's fix established for invalid status filters, and the current frontend only sends valid `JobState`s.*

**The 2026-06-11 sweep (native mac app, `native/mac` — two P1, five P2, two RG, two UX, one TD, one FG): all RESOLVED.**

The entire native-mac sweep is closed. `just mac-check` (the `AlchemistMacChecks`
harness — including the new `sseParserSplitsFramesFromRawBytes` and
`bundledModeUsesPrivatePort` checks — then `swift build`) passes.

1. **[P1-12] SSE framing** — **RESOLVED.** Byte→line framing moved into
   `AlchemistSSEParser`; `streamEvents` iterates raw bytes, yields a synthetic
   `.connected` marker, and maps 401→`.unauthorized`.
2. **[P1-13] Undrained daemon pipes** — **RESOLVED.** stdout/stderr stream to
   `daemon.log`; `recentLogLines()` surfaced in SystemView.
3. **[P2-37] + [TD-13]** — **RESOLVED.** One `bootstrap()` with a readiness poll; keychain
   token dropped only on real `.unauthorized`.
4. **[P2-36] + [P2-40]** — **RESOLVED.** Quit-time daemon termination, crash
   `terminationHandler`, private port 41737, `/api/ready` adopt/readiness probe.
5. **[P2-38] + [P2-39]** — **RESOLVED.** Honest reconnect state machine (stops on 401,
   real backoff) and tab/sort/page `.onChange` refetch.
6. **[RG-12], [RG-13], [UX-6], [UX-7], [FG-6]** — **RESOLVED.** Cookie-free session, live
   inspector refresh, ⌘⌥S/P shortcuts, cancelled-notification copy, and fully wired
   remote mode.

*Deferred follow-up (not blocking): strict daemon version-matching on adopt needs an
auth-free version field on `/api/ready`; until then the private port 41737 carries the
anti-collision weight.*

**Current release gate status, 2026-06-13:** local `just release-verify` passes with the
installed toolchain (`~/.bun/bin/bun`, `/usr/local/share/dotnet/dotnet`,
`/opt/homebrew/bin/{just,gh,ffmpeg,ffprobe,actionlint}`). `docs` is cleared outright; the
remaining `web` `esbuild` advisories are temporarily ignored in `run_bun_audit.py`
because the current compatible Astro/Vite line still pins `esbuild ^0.27.x` and the
broader build-chain migration is deferred.

**The 2026-06-13 readiness sweep (one P2, one RG, one UX): all RESOLVED in the current
worktree.**

1. **[P2-41] Setup wizard hardware wait can deadlock** — **RESOLVED.** Setup now polls
   `/api/system/hardware` directly and the pending hardware path is covered by Playwright.
2. **[RG-14] Windows release smoke cleanup is still flaky** — **RESOLVED.** The smoke
   harness no longer depends on context-manager temp cleanup and local smoke passes end to
   end after the rewrite.
3. **[UX-8] Library preview errors are swallowed** — **RESOLVED.** Preview failures are
   shown inline and block progression until fixed.

**The 2026-06-22 sweep (frontend audit + live VideoToolbox failure log — one P1, one P2,
one RG, one UX): all RESOLVED.**

1. **[P1-14] VideoToolbox probe/runtime mismatch + unsupported `-q:v` + Transient retry** —
   **RESOLVED.** Probe is now hardware-only (matches the real command), a one-time CPU
   fallback recovers encoder-open failures, and `map_failure` reclassifies them as
   `EncoderUnavailable` (not Transient). New tests cover the probe, the CPU fallback plan,
   and the classifier.
2. **[P2-42] Convert uploads aborted at 30 s by `apiFetch`** — **RESOLVED.** Per-call
   `timeoutMs` on `apiFetch`; `ConversionTool` uploads via XHR with no timeout + progress.
3. **[RG-15] Convert status polls every 2 s forever** — **RESOLVED.** Poll stops on
   terminal status; persistent errors surface a toast.
4. **[UX-9] Save View uses `window.prompt`** — **RESOLVED.** Replaced with `SaveViewDialog`.

**Beyond the four findings, this round also delivered the user's error-handling and logging
overhaul (not previously tracked as audit entries):**

- **Central error-code catalog + docs links.** Every `Explanation` and `AlchemistError`
  now carries a stable code and a `docs_url` (`{base}/errors#<code>`), surfaced in API
  problem+json (`code`/`docs_url`), `ApiError`, and a "Learn more" link in the job-detail
  failure panel. New docs page `docs/docs/errors.md` documents every code.
- **Logging:** daily-rotating file via `tracing-appender` (`runtime::log_dir()`), per-job
  `info_span!("job", job_id)` so logs are traceable end-to-end, a secret-redaction pass on
  the log store (`src/redact.rs`, applied in `db.add_log`), and a "Download logs" button +
  `/api/logs/download` endpoint.
- **Errors:** bounded transient retry with capped backoff in the processor (deterministic
  failures fail fast via `AlchemistError::is_retryable`), and the opaque
  "Unknown error: Transient" log replaced with a coded message.
