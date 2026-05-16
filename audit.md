# Audit Findings

Last updated: 2026-05-15

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

## Feature Gaps

---

### [FG-4] Intelligence page content not actionable

**Status: RESOLVED**

---

### [FG-5] Duplicate intelligence misses same-title files when the container or extension differs

**Status: RESOLVED**

---

## What To Fix First

**The 2026-05-14 round (four P2s, one TD, one UX) is fully addressed.** P2-28 file_id verification, P2-29 Windows LIKE escaping, P2-30 preview single-flight + cap + path bound, P2-31 archived-job filter on reason trends, TD-11 `device_id_for` off the runtime, and UX-5 theme-flash are all resolved with regression tests; `just check-rust` and the full lib suite (247 tests) pass.

**The 2026-05-15 sweep (one P2, one TD):**

1. **[P2-32] Notification SSRF guard misses IPv4-mapped IPv6** — **RESOLVED.** `is_private_ip` now normalizes `::ffff:a.b.c.d` via `to_ipv4_mapped()` so mapped internal addresses can no longer slip past `build_safe_client`.
2. **[TD-12] Dead `_unused_ensure_public_endpoint`** — **RESOLVED.** The ~36-line dead function was deleted.

**The entire 2026-05-15 sweep is now closed.** `just check-rust` and the full lib suite (248 tests) pass.
