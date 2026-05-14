# Audit Findings

Last updated: 2026-05-13

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

**Files:**
- `src/server/system.rs:401–422` — `reanalyze_library_root_handler` loops over watch roots with `if let Ok(count) = state.db.reanalyze_jobs_under_path(&root).await`, dropping `Err` cases on the floor.

**Severity:** P2

**Problem:**

```rust
let mut total_reanalyzed = 0;
for root in root_paths {
    if let Ok(count) = state.db.reanalyze_jobs_under_path(&root).await {
        total_reanalyzed += count;
    }
}
axum::Json(serde_json::json!({ "count": total_reanalyzed })).into_response()
```

If two of three roots fail (locked DB, schema bug, transient I/O), the user sees a 200 OK with `count = 17` and assumes the operation succeeded everywhere. There is no warning surface, no structured response, and no log line.

**Fix:**

1. Capture per-root outcomes and emit them in the response:
   ```rust
   let mut count = 0_i64;
   let mut errors: Vec<String> = Vec::new();
   for root in root_paths {
       match state.db.reanalyze_jobs_under_path(&root).await {
           Ok(n) => count += n,
           Err(err) => {
               tracing::error!(root = %root, "reanalyze_jobs_under_path failed: {err}");
               errors.push(format!("{root}: {err}"));
           }
       }
   }
   if !errors.is_empty() && count == 0 {
       return api_error_response(StatusCode::INTERNAL_SERVER_ERROR,
           "REANALYZE_FAILED", errors.join("; "));
   }
   axum::Json(serde_json::json!({ "count": count, "errors": errors })).into_response()
   ```
2. Update the OpenAPI spec for `/api/v1/library/reanalyze` to document the optional `errors` field.

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

## Feature Gaps

---

### [FG-4] Intelligence page content not actionable

**Status: RESOLVED**

---

### [FG-5] Duplicate intelligence misses same-title files when the container or extension differs

**Status: RESOLVED**

---

## What To Fix First

**All items in this section have been completed.**

The 2026-05-12 round (one P1, four P2s, two TDs, one RG) is fully addressed: data-loss surface in `purge_jobs_by_filter` is closed, the library health scan flag is now consulted, stats queries filter archived rows, concurrent update installs return 409, reanalyze surfaces per-root failures, blocking syscalls in async paths moved off the runtime, and update archive downloads stream to disk with RAII cleanup.
