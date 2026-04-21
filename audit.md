# Audit Findings

Last updated: 2026-04-21

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

### [P1-3] Notification target migration rewrites the live table instead of evolving it additively

**Status: RESOLVED**

**Files:**
- `migrations/20260407110000_notification_targets_v2_and_conversion_jobs.sql:1–34` — replaces `notification_targets` by copying rows into a new table, then dropping and renaming
- `migrations/20260109240000_notifications.sql:1–10` — original `notification_targets` schema that the replacement migration destroys

**Severity:** P1

**Problem:**

The `20260407110000_notification_targets_v2_and_conversion_jobs.sql` migration violates the repo's additive-only schema rule by creating a replacement table, copying data, then dropping and renaming the original table. That is not just a style violation: if the migration aborts between the copy and rename steps, the user's configured notification targets are gone, and the upgrade path no longer preserves the previous schema for mixed-version or rollback scenarios.

```sql
INSERT INTO notification_targets_new (...)
SELECT ... FROM notification_targets;

DROP TABLE notification_targets;
ALTER TABLE notification_targets_new RENAME TO notification_targets;
```

**Fix:**

Replaced the migration with an additive change on `notification_targets`: `target_type_v2` and `config_json` are added in place, backfilled, and legacy `endpoint_url` / `auth_token` are retained for compatibility. Runtime reads/writes in `src/db/config.rs` now prefer the v2 shape while still tolerating legacy columns, and `tests/integration_db_upgrade.rs` verifies upgraded notification targets survive with the same semantics.

---

### [P1-4] Deleting one duplicate settings row could delete all matching rows

**Status: RESOLVED**

**Files:**
- `src/server/settings.rs:652–680` — notification deletion used content-based `retain` after looking up a single row by id
- `src/server/settings.rs:847–876` — schedule deletion used the same content-based removal pattern
- `src/db/config.rs:420–459` — notification/schedule projections now load in deterministic `id ASC` order for index mapping
- `src/server/tests.rs:1458–1543` — regression tests verify deleting one duplicate leaves the other intact

**Severity:** P1

**Problem:**

`delete_notification_handler()` and `delete_schedule_handler()` used the database id only to find *one* row, then removed config entries by matching on the row contents. Because the TOML config does not store stable ids, duplicate notification targets or duplicate schedule windows were all considered the same entry and were deleted together. That is direct configuration data loss from a single delete action.

```rust
next_config.notifications.targets.retain(|candidate| {
    !(candidate.name == target.name
        && candidate.target_type == target.target_type
        && candidate.config_json == parsed_target_config_json)
});
```

**Fix:**

1. In `src/db/config.rs`, make `get_notification_targets()` and `get_schedule_windows()` return rows in deterministic `id ASC` order so DB ids map predictably back to config-array positions.
2. In `src/server/settings.rs`, resolve the selected row's index from the ordered DB projection and call `.remove(index)` on the corresponding config array instead of value-based `retain`.
3. In `src/server/settings.rs`, return the last projected row after add operations so duplicate names/times do not echo the wrong record back to the UI.
4. In `src/server/tests.rs`, keep regression tests for duplicate notification targets and duplicate schedule windows so future refactors cannot reintroduce multi-delete behavior.

---

### [P1-5] Conversion expiry cleanup can delete active transcodes and their artifacts

**Status: RESOLVED**

**Files:**
- `src/server/conversion.rs:52–63` — expired conversion jobs are deleted unconditionally, including their upload/output files
- `src/db/conversion.rs:141–149` — `get_expired_conversion_jobs()` returns every row whose `expires_at` is in the past
- `src/server/conversion.rs:233–289` — started conversion jobs keep using the uploaded temp file and store the linked transcode job id here
- `src/media/pipeline.rs:1469–1476` — manual-conversion settings are only applied while the `conversion_jobs` row still exists

**Severity:** P1

**Problem:**

Every conversion endpoint calls `cleanup_expired_jobs()` before doing any other work, and the expiry query only checks `expires_at <= now`. `expires_at` is set once at upload time and never refreshed, so a long-running or delayed conversion becomes eligible for cleanup while its linked transcode is still queued or encoding. When that happens, the server deletes the upload file and removes the `conversion_jobs` row; if the linked job has not reached planning yet, the pipeline falls back to normal library planning because it can no longer load the saved conversion settings.

```rust
for job in expired {
    let _ = remove_conversion_artifacts(&job).await;
    let _ = state.db.delete_conversion_job(job.id).await;
}
```

**Fix:**

1. In `src/db/conversion.rs`, change `get_expired_conversion_jobs()` so active linked jobs are never returned for cleanup. The query should `LEFT JOIN jobs` on `linked_job_id` and exclude rows whose linked job is still in `queued`, `analyzing`, `encoding`, `remuxing`, or `resuming`.
2. In `src/server/conversion.rs`, refresh the retention window when `start_conversion_job_handler()` links a real transcode job so the TTL is measured from job start or completion, not from the original upload timestamp.
3. In `src/media/pipeline.rs`, when a linked conversion job reaches a terminal state, extend or recalculate its conversion-job expiry from that completion time so the download window is stable even for long encodes.
4. Add a regression test that starts a conversion job, advances/forces expiry, runs `cleanup_expired_jobs()`, and verifies the linked upload file, conversion row, and saved conversion settings all survive while the linked job is active.

---

### [P1-6] Manual conversion jobs silently fall back to library planning on conversion-row lookup failure

**Files:**
- `src/media/pipeline.rs:1469–1476` — linked conversion rows are loaded with `.await.ok().flatten()`, so database failures are treated as “not a manual conversion”
- `src/db/conversion.rs:42–54` — `get_conversion_job_by_linked_job_id()` already returns a real `Result<Option<ConversionJob>>`
- `src/server/conversion.rs:368–424` — `start_conversion_job_handler()` explicitly creates the linked job and stores the conversion settings row that planning is supposed to consume

**Severity:** P1

**Problem:**

Manual conversions depend on the `conversion_jobs` row to recover the user-selected container, codec, remux flag, and other overrides during planning. In `pipeline.rs`, that lookup is wrapped in `.ok().flatten()`, so any SQLite error is silently converted into `None` and the job is planned like a normal library transcode instead of failing. That is a wrong-output bug: the request can return `200`, the transcode can complete, and the produced file can ignore the settings the user just previewed and approved.

```rust
let conversion_job = self
    .db
    .get_conversion_job_by_linked_job_id(job.id)
    .await
    .ok()
    .flatten();
```

**Fix:**

1. In `src/media/pipeline.rs`, replace the `.ok().flatten()` path with an explicit `match` on `get_conversion_job_by_linked_job_id(job.id).await`.
2. If the lookup returns `Err`, record a job log / failure explanation and fail the job instead of falling back to normal planner behavior.
3. Keep `Ok(None)` as the non-manual path, but only after distinguishing it from real database failures.
4. Add a regression test that injects a database failure into `get_conversion_job_by_linked_job_id()` for a linked conversion job and verifies the job fails loudly instead of using default library planning.

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

### [P2-8] Finalization reprobes the input file instead of the encoded output

**Status: RESOLVED**

**Files:**
- `src/media/pipeline.rs:1471–1479` — duration fallback path uses `input_path` while logging that it is probing the output

**Severity:** P2

**Problem:**

`finalize_job()` is supposed to recover a missing duration before saving encode stats, but when `context.metadata.duration_secs <= 0.0` it reprobes `input_path` instead of the encoded artifact. That means files whose input metadata is missing duration still save `encode_speed` and bitrate from the wrong file, and they can remain zeroed even when the transcode output has a perfectly valid duration. The logging text already says "reprobe output", which makes the bug easy to miss in reviews.

```rust
if media_duration <= 0.0 {
    match crate::media::analyzer::Analyzer::probe_async(input_path).await {
        Ok(meta) => {
            media_duration = meta.format.duration.parse::<f64>().unwrap_or(0.0);
        }
```

**Fix:**

Finalization now reprobes the encoded artifact (`temp_output_path` before promotion, otherwise `output_path`) when stored input duration is missing. The warning logs the actual reprobed path, and a regression test verifies encode stats are computed from the encoded output duration rather than the source file.

---

### [P2-9] Job detail handler turns database failures into empty sections and still returns 200

**Status: RESOLVED**

**Files:**
- `src/server/jobs.rs:383–389` — completed-job encode stats are fetched with `.ok()`
- `src/server/jobs.rs:427–435` — encode attempts and queue position use `unwrap_or_default()` / `unwrap_or(None)`

**Severity:** P2

**Problem:**

`get_job_detail_handler()` suppresses several database errors and still returns a successful JSON payload. When `encode_stats`, `encode_attempts`, or `queue_position` queries fail, the frontend sees a normal 200 response with missing sections and renders "No encode data available" or no queue position, even though the backend is unhealthy. That hides operational failures and makes users trust incomplete job details as if they were authoritative.

```rust
let encode_stats = if job.status == JobState::Completed {
    state.db.get_encode_stats_by_job_id(id).await.ok()
} else {
    None
};
```

**Fix:**

`src/server/jobs.rs` now treats only `RowNotFound` as optional-missing data for completed encode stats, and returns `500` on real DB failures for encode stats, attempt history, and queue position. A handler regression test verifies the endpoint no longer returns partial `200 OK` payloads when the backing queries fail.

---

### [P2-10] `%` and `_` in watch folder paths can assign the wrong library profile

**Status: RESOLVED**

**Files:**
- `src/db/config.rs:287–307` — profile lookup uses `LIKE` with the stored watch path as the pattern
- `src/db/config.rs:309–320` — strict ancestry fallback never runs after a false-positive SQL match

**Severity:** P2

**Problem:**

`get_profile_for_path()` builds its fast-path lookup with `? LIKE wd.path || '/%'`. Because `wd.path` is inserted into the SQL pattern unescaped, any watch directory containing `%` or `_` is treated as a wildcard pattern. A folder like `/media/TV_4K` can therefore match unrelated siblings and return the wrong `LibraryProfile`, which means the planner can apply the wrong codec, HDR mode, or quality preset to media outside the intended tree.

```sql
WHERE wd.profile_id IS NOT NULL
  AND (? = wd.path OR ? LIKE wd.path || '/%' OR ? LIKE wd.path || '\\%')
```

**Fix:**

The SQL fast path in `src/db/config.rs` now uses literal prefix checks instead of wildcard-prone `LIKE` matching, and the strict `Path::starts_with` fallback remains only for normalization edge cases. Regression tests cover watch directories containing `%` and `_` and confirm longest literal-path matching still wins.

---

### [P2-11] Login collapses database errors into “invalid credentials”

**Status: RESOLVED**

**Files:**
- `src/server/auth.rs:34–39` — user lookup uses `unwrap_or(None)` and discards DB errors
- `src/server/auth.rs:72–73` — the handler returns `401 Unauthorized` after the swallowed error path

**Severity:** P2

**Problem:**

`login_handler()` intentionally does dummy Argon2 work to equalize timing for bad usernames, but it also uses the same path for real database failures. If SQLite is locked, unavailable, or corrupt, `get_user_by_username()` is collapsed into `None` and the caller gets `Invalid credentials` instead of a server error. That turns an operator-visible auth outage into a misleading credential error and makes monitoring or client automation misclassify backend failures as user mistakes.

```rust
let user_result = state
    .db
    .get_user_by_username(&payload.username)
    .await
    .unwrap_or(None);
```

**Fix:**

`src/server/auth.rs` now matches `get_user_by_username()` explicitly: missing users still go through the dummy-hash path, but real database errors are logged and returned as `500` instead of being misreported as invalid credentials. A server test covers the lookup-failure path.

---

### [P2-12] Job SSE reconciliation leaves filtered tables and the detail modal stale

**Status: RESOLVED**

**Files:**
- `web/src/components/jobs/useJobSSE.ts:5–137` — SSE hook patched local rows in place but did not reconcile filtered lists or refresh focused details after status/decision lag
- `web/src/components/JobManager.tsx:39–41` — focused-job refs now let the SSE hook refresh the currently open detail view
- `web/src/components/JobManager.tsx:220–240` — the manager wires the focused-job refresh callback into the SSE layer

**Severity:** P2

**Problem:**

The jobs UI relied on SSE for live updates, but `useJobSSE()` only patched the current in-memory list and mostly ignored the shape of the active view. When a queued job moved to encoding or completion, filtered tabs could keep showing that row until the next polling cycle, and an open detail modal only had its status field patched while queue position, encode stats, logs, and explanations stayed stale until the operator closed and reopened it. The hook also ignored `lagged` events from the backend, so missed SSE messages could leave the UI behind server truth.

```ts
eventSource.addEventListener("status", (e) => {
    setJobs((prev) =>
        prev.map((job) => job.id === job_id ? { ...job, status } : job)
    );
    setFocusedJob((prev) =>
        prev?.job.id === job_id ? { ...prev, job: { ...prev.job, status } } : prev
    );
});
```

**Fix:**

1. In `web/src/components/jobs/useJobSSE.ts`, refetch the current jobs table on status, decision, and lagged events so filtered tabs reconcile against server truth instead of keeping rows that no longer belong.
2. In `web/src/components/jobs/useJobSSE.ts`, update focused-job progress inline and refresh the focused job detail payload whenever the relevant job emits a status or decision event.
3. In `web/src/components/JobManager.tsx`, keep refs for the currently focused job id and a refresh callback so the SSE hook can refresh detail state without re-subscribing on every render.
4. Keep frontend typechecking green after the new SSE wiring so the modal/job-detail contracts stay consistent.

---

### [P2-13] Conversion upload buffers the entire video into memory

**Status: RESOLVED**

**Files:**
- `src/server/conversion.rs:79–105` — upload handler calls `field.bytes().await` and then writes the whole payload in one shot
- `src/server/mod.rs:355–367` — the conversion upload route has no dedicated streaming or size-limit guard around it

**Severity:** P2

**Problem:**

`upload_conversion_handler()` reads the multipart file into a single in-memory `Bytes` buffer before writing it to disk. That is safe for tiny samples, but the endpoint is explicitly for video uploads, so a multi-gigabyte file can force the server to allocate the entire payload at once and get killed by memory pressure before the write even starts. Because the route has no route-specific body limit or chunked write path, large uploads are effectively an OOM footgun.

```rust
match field.bytes().await {
    Ok(bytes) => {
        if let Err(err) = fs::write(&path, bytes).await {
            return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
        }
    }
```

**Fix:**

1. In `src/server/conversion.rs`, replace `field.bytes().await` with a streamed write loop using `tokio::fs::File` plus repeated `field.chunk().await` calls so memory usage stays bounded by chunk size.
2. In `src/server/mod.rs`, add an explicit request-size limit for `/api/conversion/uploads` so pathological uploads fail with a clear `413` instead of exhausting process memory.
3. Add an integration test that uploads a payload larger than the configured limit and verifies the server rejects it cleanly without leaving a partial conversion row behind.

---

### [P2-14] Conversion preview can return 200 even when the saved settings were not persisted

**Status: RESOLVED**

**Files:**
- `src/server/conversion.rs:183–208` — preview handler ignores failures from probe/status/settings DB writes and still returns the preview
- `src/server/conversion.rs:255–264` — start handler later reloads `settings_json` from the database to build the real output path and encode plan

**Severity:** P2

**Problem:**

The preview endpoint normalizes the requested settings and returns a command preview, but it discards errors from all three persistence writes that are supposed to save that normalized state. If any of those updates fail, the client still sees `200 OK` and assumes the draft is saved, while `start_conversion_job_handler()` later deserializes whatever stale `settings_json` was already in the database. That can make the actual conversion run with different codec/container/settings than the preview the user just approved.

```rust
let _ = state.db.update_conversion_job_status(job.id, ...).await;
let _ = sqlx_update_conversion_settings(state.as_ref(), job.id, &preview.normalized_settings).await;
axum::Json(preview).into_response()
```

**Fix:**

1. In `src/server/conversion.rs`, make `preview_conversion_handler()` propagate database write failures instead of ignoring them; a failed draft save should return `500`, not a successful preview.
2. Wrap the probe/status/settings updates in one transaction or helper so the draft moves atomically from uploaded -> previewed state.
3. Keep `start_conversion_job_handler()` reading from persisted state, but add a regression test that injects a failed preview write and verifies the handler no longer returns `200 OK` with unsaved settings.

---

### [P2-15] Engine mode requests can fail persistently but still change the live runtime

**Files:**
- `src/server/jobs.rs:744–772` — `set_engine_mode_handler()` mutates `state.agent` and the shared in-memory config before it calls `save_config_or_response()`
- `src/server/mod.rs:586–618` — config persistence can legitimately fail when config writes are disabled or the config path is not writable

**Severity:** P2

**Problem:**

`set_engine_mode_handler()` applies the new mode or manual override to the live engine first, then mutates `state.config`, and only afterwards tries to persist the new config. If `save_config_or_response()` fails, the route returns an error but the process is already running with the new concurrency limit and mode. That creates a split-brain state where the user is told the update failed while the runtime has in fact changed, and the next restart snaps back to the old persisted configuration.

```rust
state.agent.set_concurrent_jobs(override_jobs).await;
*state.agent.engine_mode.write().await = payload.mode;
…
if let Err(e) = super::save_config_or_response(&state, &config).await {
    return *e;
}
```

**Fix:**

1. In `src/server/jobs.rs`, build a cloned `next_config` first instead of mutating `state.config` in place.
2. Call `save_config_or_response()` with that staged config before mutating `state.agent` or swapping the shared config.
3. Only after persistence succeeds should the handler update `state.config`, `state.agent.engine_mode`, `manual_override`, and the live concurrent-job limit.
4. Add a regression test that runs this handler with `config_mutable = false` (or an unwritable config path) and verifies both the response is an error and the live engine limit/mode remain unchanged.

---

### [P2-16] Auth middleware turns session and API-token database failures into fake 401s

**Files:**
- `src/server/middleware.rs:123–146` — protected requests use `if let Ok(Some(...))` for both session and API-token lookup paths
- `src/db/system.rs:128–189` — `get_session()` and `get_active_api_token()` return `Result`, so lookup failures are distinguishable from missing credentials

**Severity:** P2

**Problem:**

The auth middleware currently treats “database lookup failed” the same as “credential not found.” If SQLite is locked, unavailable, or otherwise errors during `get_session()` or `get_active_api_token()`, the middleware falls through to `401 Unauthorized` instead of returning a server error. That misclassifies backend outages as authentication failures, causes clients to think they need to log in again, and hides operational auth problems from monitoring.

```rust
if let Some(t) = token {
    if let Ok(Some(_session)) = state.db.get_session(&t).await {
        return next.run(req).await;
    }
    if let Ok(Some(api_token)) = state.db.get_active_api_token(&t).await {
```

**Fix:**

1. In `src/server/middleware.rs`, replace both `if let Ok(Some(...))` branches with explicit `match` handling.
2. Preserve the current `None => unauthorized` behavior for missing credentials, but return `500` on real database errors from either lookup.
3. Log the failing lookup path so operators can tell whether session auth or API-token auth is unhealthy.
4. Add middleware tests that inject a locked/failed database lookup and verify protected routes return `500` instead of `401`.

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

`src/media/pipeline.rs` now routes audit-trail writes through explicit warn-logged helpers for decisions, encode attempts, failure explanations, logs, and stored input metadata. Failure-injection tests verify the pipeline still reaches the intended primary outcome when those audit writes fail.

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

**Status: RESOLVED**

**Files:**
- `src/main.rs:320`
- `src/media/processor.rs:255`

**Severity:** RG

**Problem:**

Interrupted encodes were left in `Encoding` state and orphaned temp files remained on disk.

**Fix:**

Implemented segment-based resumable transcodes for encode jobs. The pipeline now persists `job_resume_sessions` and `job_resume_segments`, skips completed segments after restart/cancel, restores progress from completed duration, concatenates completed segments back into the normal `temp_output_path`, and preserves resume temp dirs across restarts until final success or explicit purge. Startup recovery now avoids deleting `*.alchemist.tmp` when an active resume session exists. This is segment-level resume, not byte-level continuation inside a partially encoded segment.

---

### [RG-2] AMD VAAPI/AMF hardware paths unvalidated

**Status: PARTIALLY RESOLVED**

**Files:**
- `src/media/ffmpeg/vaapi.rs`
- `src/media/ffmpeg/amf.rs`

**Severity:** RG

**Problem:**

Hardware paths for AMD (VAAPI on Linux, AMF on Windows) were implemented without real hardware validation.

**Fix:**

Added stronger always-on unit coverage around VAAPI/AMF command generation plus hardware-gated smoke tests in `tests/integration_ffmpeg.rs` that activate only when `ALCHEMIST_TEST_AMD_VAAPI_DEVICE` or `ALCHEMIST_TEST_AMD_AMF=1` is provided. Real AMD hardware sign-off is still outstanding.

---

### [RG-3] Daily summary scheduling can miss a day, suppress retries, and duplicate after restart

**Status: RESOLVED**

**Files:**
- `src/notifications.rs:233–239` — summary worker polls every 30 seconds from process start rather than from the next scheduled minute
- `src/notifications.rs:321–330` — `daily_summary_last_sent` is marked before any fetch or delivery succeeds
- `src/notifications.rs:332–346` — delivery failures are logged after the day has already been marked as sent

**Severity:** RG

**Problem:**

Daily summaries are triggered by a relative `sleep(30s)` loop plus an exact `hour/minute` comparison. If the process starts after the configured minute, the first check can happen in the next minute and the summary is skipped for the whole day. When a check does happen, the code marks that date as sent before loading the summary or contacting any target, so a transient DB error or one-shot network failure prevents retries for the rest of the day; after restart, the in-memory flag resets and the same day's summary can be sent again.

```rust
tokio::time::sleep(Duration::from_secs(30)).await;
if let Err(err) = summary_manager.maybe_send_daily_summary().await { ... }

*last_sent = Some(summary_key.clone());
let summary = self.db.get_daily_summary_stats().await?;
```

**Fix:**

Daily summaries now run from a minute-aligned scheduler, only mark the day as sent after a successful delivery or an explicit no-targets decision, and persist the last successful date in preferences so restarts do not duplicate or forget the same day. Regression tests cover retry-after-failure, restart safety, and the no-eligible-targets path.

---

### [RG-4] Library health scans still probe archived jobs

**Status: RESOLVED**

**Files:**
- `src/db/jobs.rs:939–956` — `get_jobs_needing_health_check()` selects completed jobs without `archived = 0`
- `src/server/scan.rs:116–153` — background scan workers probe every returned row and write fresh health results back to the database
- `src/db/system.rs:297–313` — the library-health issue listing already excludes archived rows, so the background work is invisible in normal UI views

**Severity:** RG

**Problem:**

The background library-health scan still treats archived jobs as eligible because `get_jobs_needing_health_check()` filters on `status = 'completed'` but never filters out soft-deleted rows. That means files the user explicitly archived can keep getting probed every scan cycle, and their `last_health_check` / `health_issues` fields continue mutating even though archived jobs are hidden from the health UI. The result is wasted disk I/O plus invisible state churn on rows that are supposed to be out of circulation.

```sql
FROM jobs j
WHERE j.status = 'completed'
  AND (
       j.last_health_check IS NULL
       OR j.last_health_check < datetime('now', '-7 days')
  )
```

**Fix:**

1. In `src/db/jobs.rs`, add `AND j.archived = 0` to `get_jobs_needing_health_check()` so the background scanner uses the same soft-delete rules as the rest of the health UI.
2. Add a regression test that archives a completed job, runs the health-scan selection query, and verifies the archived row is excluded.
3. Optionally backfill a one-time cleanup for archived rows with stale `health_issues` if you want archived-job metadata to stop surfacing in direct DB inspection.

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

**Status: RESOLVED**

**Severity:** UX

**Problem:**

Jobs only enter the queue via full library scans. No manual "Enqueue path" exists in the UI.

**Fix:**

Added `POST /api/jobs/enqueue` plus a modal-backed "Add file" action in `JobsToolbar`. The endpoint reuses `enqueue_discovered_with_db()` so manual enqueue follows the same output, skip, and dedupe rules as scan-driven intake.

---

### [UX-3] Workers-blocked reason not surfaced for queued jobs

**Status: RESOLVED**

**Severity:** UX

**Problem:**

Users cannot see why a job is stuck in Queued (paused, scheduled, or slots full).

**Fix:**

Added `GET /api/processor/status` with precedence-aware blocked reasons (`manual_paused`, `scheduled_pause`, `draining`, `workers_busy`), and the job detail modal now shows the returned reason for queued jobs.

---

## Feature Gaps

---

### [FG-4] Intelligence page content not actionable

**Status: RESOLVED**

**Files:**
- `web/src/components/LibraryIntelligence.tsx`

**Severity:** FG

**Problem:**

Intelligence page is informational only; recommendations cannot be acted upon directly from the page.

**Fix:**

The Intelligence page now supports "Queue all" for remux opportunities and per-duplicate "Review" actions that open the shared job-detail modal directly from the page.

---

## What To Fix First

1. **[P1-6]** Make manual conversion planning fail loudly on `conversion_jobs` lookup errors instead of silently falling back to normal library rules.
2. **[P2-15]** Reorder engine-mode updates so persistence succeeds before the live runtime and shared config are mutated.
3. **[P2-16]** Return `500` from auth middleware on session/token lookup failures so backend auth outages are not misreported as bad credentials.
4. **[RG-2]** Run the gated VAAPI/AMF smoke tests on real AMD hardware and confirm or adjust the runtime flags if they fail.
