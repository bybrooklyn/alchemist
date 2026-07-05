# Super Audit — Alchemist

Multi-agent deep audit run against `v0.3.5-rc.1` (commit `c1480bcc`). Eight domain auditors read the actual source; findings below were reported with concrete failure scenarios and file:line locations. Frontend audited and found **clean**.

## Summary

| # | Severity | Domain | Finding | Location |
|---|----------|--------|---------|----------|
| 1 | 🔴 high | pipeline | Duplicate output paths → shared temp corruption + source data loss | `src/media/pipeline.rs:1299`, `processor.rs:194` |
| 2 | 🔴 high | ffmpeg/orch | Lock-order inversion deadlock (`cancel_channels` vs `pending_cancels`) | `src/orchestrator.rs:103,293` |
| 3 | 🔴 high | ffmpeg/orch | `&line[..4096]` panics on non-UTF-8 char boundary | `src/orchestrator.rs:327` |
| 4 | 🔴 high | server/auth | X-Forwarded-For leftmost trusted → setup-gate + rate-limit bypass | `src/server/middleware.rs:453` |
| 5 | 🔴 high | secrets | Notification failures leak webhook/bot tokens into downloadable log | `src/notifications.rs:373`, `src/main.rs:1436` |
| 6 | 🔴 high | concurrency | Config read-lock held across VMAF stalls all config readers process-wide | `src/media/pipeline.rs:2232` |
| 7 | 🟠 medium | pipeline | Conversion jobs skip zero-byte / size-integrity gate before promoting | `src/media/pipeline.rs:2235` |
| 8 | 🟠 medium | ffmpeg/orch | Spawned FFmpeg child not killed on future drop (orphan/leak) | `src/orchestrator.rs:273`, `ffmpeg/mod.rs:177` |
| 9 | 🟠 medium | ffmpeg/orch | Subtitle-burn filtergraph escaping mishandles single quotes (injection) | `src/media/ffmpeg/mod.rs:483,503` |
| 10 | 🟠 medium | db | Missing index on `jobs.output_path` → quadratic library scans | `src/db/jobs.rs:1285` |
| 11 | 🟠 medium | db | Daily-boundary queries use wrong SQLite modifier order → wrong "today" stats | `src/db/stats.rs:424` |
| 12 | 🟠 medium | secrets | Email (SMTP) notification target bypasses SSRF / `allow_local` guard | `src/notifications.rs:82,908` |
| 13 | 🟠 medium | system/fs | Windows `\\?\` verbatim prefix breaks device-id grouping | `src/system/device_id.rs:14,38` |
| 14 | 🟠 medium | system/fs | `classify_media_hint` amplifies each browse into O(N) recursive walks | `src/system/fs_browser.rs:143,519` |
| 15 | 🟠 medium | concurrency | Lost-update race in config read-modify-write settings handlers | `src/server/settings.rs:82` (+ others) |
| 16 | 🟡 low | pipeline | Cancel during finalize mislabels completed job as Cancelled, still deletes source | `src/media/pipeline.rs:2130` |
| 17 | 🟡 low | pipeline | Windows output promotion non-atomic (remove-then-rename) | `src/media/pipeline.rs:2199` |
| 18 | 🟡 low | ffmpeg/orch | Paths via `display().to_string()` corrupt non-UTF-8 filenames | `src/media/ffmpeg/mod.rs:188` |
| 19 | 🟡 low | ffmpeg/orch | NVENC VBR+CQ without `-b:v 0` can cap quality | `src/media/ffmpeg/nvenc.rs:16` |
| 20 | 🟡 low | ffmpeg/orch | QSV `-look_ahead 20` is a boolean flag, not the depth | `src/media/ffmpeg/qsv.rs:33` |
| 21 | 🟡 low | db | `VACUUM INTO` interpolates backup path directly into SQL | `src/db/system.rs:358` |
| 22 | 🟡 low | db | Missing index on `logs.job_id` | `src/db/system.rs:59` |
| 23 | 🟡 low | server/auth | Login endpoint leaks raw DB error strings to unauthenticated callers | `src/server/auth.rs:46` |
| 24 | 🟡 low | server/auth | Setup token compared non-constant-time and not URL-decoded | `src/server/middleware.rs:118` |
| 25 | 🟡 low | secrets | `redact.rs` webhook masking uses `rfind('/')`, leaks Discord `/slack` `/github` token | `src/redact.rs:132` |
| 26 | 🟡 low | secrets | Schedule window `start_time == end_time` silently never activates | `src/scheduler.rs:99`, `config.rs:1107` |
| 27 | 🟡 low | system/fs | Disk guardrail fails open for verbatim/mismatched-prefix output paths | `src/system/disk_space.rs:26` |
| 28 | 🟡 low | system/fs | `fs_browser` sensitive-path blocklist is advisory / incomplete | `src/system/fs_browser.rs:336` |

**Totals:** 6 high · 9 medium · 13 low · 28 total. Frontend: 0 real defects.

---

## 🔴 High severity

### 1. Duplicate output paths → shared temp corruption + source data loss
- **category:** data-loss / concurrency
- **location:** `src/media/pipeline.rs:1299` (`temp_output_path_for`), `1277-1291` (`skip_reason_for_discovered_path`), `src/media/processor.rs:194-223` (`scan_and_enqueue` batch), `migrations/20231026000000_initial_schema.sql:5`
- **problem:** `output_path_for_source` (`src/db/types.rs:360`) derives the output name from the input's `file_stem()` (extension dropped) + suffix + `output_extension`. Two sources with the same stem but different extensions collide on one output path. `jobs.output_path` is **not** UNIQUE (only `input_path` is), and the only guard `db.has_job_with_output_path` is checked against the DB — but `scan_and_enqueue` buffers up to `ENQUEUE_CHUNK = 500` rows before writing, so two colliding files in one chunk both pass (neither is in the DB yet) and both enqueue. The temp path is derived solely from the output: `parent.join(format!("{filename}.alchemist.tmp"))` — both jobs share one temp file. The comment claiming "same-file concurrent transcodes are prevented at the job level" is keyed on `input_path`, not `output_path`.
- **impact:** `Show.S01E01.mkv` and `Show.S01E01.mp4` in one folder both map to `Show.S01E01-alchemist.mkv`. They sort adjacently, land in one scan chunk, both enqueue. With `concurrent_jobs >= 2` both write `…​.alchemist.tmp` simultaneously (and each may `remove_file` the other's temp) → corrupted output promoted and marked Completed. With `replace_strategy = replace` + `delete_source = true`, both delete their distinct sources while only one output survives → **outright loss of one title.**
- **fix:** Dedup colliding output paths within the scan buffer before `enqueue_jobs_batch`; make the temp path unique per job (include `job.id`: `{filename}.alchemist.{job_id}.tmp`) so two jobs can never share a temp file; add a non-unique index on `jobs.output_path` (see #10 — a UNIQUE constraint would require a table rebuild, forbidden by additive-only). Gate promotion on an output-path lock.

### 2. Lock-order inversion deadlock between `cancel_job` and `run_ffmpeg_command`
- **category:** concurrency
- **location:** `src/orchestrator.rs:103-127` and `293-306`
- **problem:** Two `std::sync::Mutex`es acquired in opposite orders on different threads. `cancel_job` locks `cancel_channels` first (line 103), then in the `None` arm locks `pending_cancels` (line 115) while still holding `cancel_channels`. `run_ffmpeg_command` locks `pending_cancels` first (line 293), then at line 301 locks `cancel_channels` while still holding `pending_cancels`. Classic AB/BA. The struct comment (lines 16-17) only claims safety against holding-across-`.await`, which does not cover cross-function ordering.
- **impact:** User cancels a queued job (thread B in `cancel_job`, holds `cancel_channels`) at the same instant a worker (thread A in `run_ffmpeg_command`, holds `pending_cancels` because a cancel was queued before spawn) runs. A blocks on `cancel_channels`, B blocks on `pending_cancels`; both blocking `std::sync::Mutex` with no timeout → the Axum handler thread and the worker thread hang permanently.
- **fix:** Establish one global lock order. Compute `pending.remove(&id)` into a bool and drop the `pending_cancels` guard before re-locking `cancel_channels`; never hold both simultaneously.

### 3. `&line[..4096]` panics on a non-UTF-8 char boundary
- **category:** correctness
- **location:** `src/orchestrator.rs:327-331`
- **problem:** `if line.len() > 4096 { format!("{}...[truncated]", &line[..4096]) }`. `line.len()` is byte length; `&line[..4096]` slices by byte index. If offset 4096 falls mid-codepoint, Rust panics (`byte index 4096 is not a char boundary`). FFmpeg stderr lines can contain non-ASCII (accented/CJK filenames, error text) and exceed 4096 bytes.
- **impact:** A long stderr line whose byte 4096 is inside a multi-byte character panics the stderr-reading task, aborting the transcode with a panic rather than a clean failure. Combined with #8, the child ffmpeg is then orphaned.
- **fix:** Truncate on a char boundary: `let end = (0..=4096).rev().find(|&i| line.is_char_boundary(i)).unwrap_or(0); &line[..end]`, or `line.chars().take(4096).collect::<String>()`.

### 4. X-Forwarded-For leftmost entry is trusted → LAN setup-gate + rate-limit bypass
- **category:** security
- **location:** `src/server/middleware.rs:453-460` (used by `request_is_lan` / `request_ip` / login)
- **problem:** When the peer is a "trusted" proxy, `resolved_client_ip` takes the *leftmost* XFF entry, which is fully attacker-controlled. The nginx idiom `$proxy_add_x_forwarded_for` appends the real client, so `X-Forwarded-For: 192.168.0.5` arrives as `192.168.0.5, <real-client>` and resolves to `192.168.0.5`. `trusted_proxies` only controls which *peer* is trusted, never which position is read. In the default config any private-range peer is trusted.
- **impact:** (1) **Setup-gate bypass** during first run — `request_is_lan` returns true for a spoofed LAN IP, so a remote attacker behind the recommended reverse proxy can reach `/api/setup/complete`, `/api/fs/browse`, `/api/settings/bundle`, create the admin account, browse the filesystem pre-auth. (2) **Login brute-force bypass** — the login/global limiters key on this IP, so rotating the leftmost XFF value gives unlimited fresh token buckets. (3) A LAN attacker can spoof XFF directly.
- **fix:** Walk the XFF chain right-to-left, discarding entries whose peer is a configured trusted proxy, take the first remaining (rightmost-untrusted) address; when `trusted_proxies` is empty, ignore forwarded headers and use the TCP peer. Never `.split(',').next()`.

### 5. Notification failures leak webhook/bot tokens into the unredacted downloadable log
- **category:** secret-leak
- **location:** `src/notifications.rs:373` (also 246, 269, 307, 449) → `src/main.rs:1432-1491` → `src/server/jobs.rs:1120`
- **problem:** On failed delivery the code logs the raw `reqwest` error: `error!("Failed to send notification to target '{}': {}", target.name, e)`. `reqwest::Error` Display embeds the full request URL. For `discord_webhook` the token is in `https://discord.com/api/webhooks/{id}/{TOKEN}`; for `telegram` it is `https://api.telegram.org/bot{TOKEN}/sendMessage`. `redact::redact_secrets` is wired **only** into the DB log store (`src/db/system.rs:38`), not the `tracing_subscriber` file/stdout layers — and `alchemist.log` is downloadable via `/api/logs/download`.
- **impact:** A Discord webhook returning 401/429, or a down Telegram target (common failure modes), writes the full secret token to `alchemist.log` in cleartext. Anyone who can reach the logs-download route recovers a working token. redact.rs explicitly targets Discord webhook URLs but is never applied on this path.
- **fix:** Log a sanitized message (status code + target name only) on notification paths, or run through `redact::redact_secrets` before emitting. Better: add a redacting `MakeWriter`/layer in `init_logging` so all tracing output (file + stdout) is redacted.

### 6. Config read-lock held across VMAF computation stalls all config readers process-wide
- **category:** concurrency
- **location:** `src/media/pipeline.rs:2232`
- **problem:** `finalize_job` takes `let config = self.config.read().await;` at line 2232 and holds the guard through VMAF `spawn_blocking(...).await` (2269-2272), `probe_async(...).await` (2332), and `save_encode_stats(...).await` (2377) — never dropped before scope end (last field access is line 2291). VMAF over a feature-length file runs for minutes. tokio's `RwLock` is write-preferring: once a writer queues, all subsequent `read()` block behind it.
- **impact:** While a `enable_vmaf` job finalizes a long encode, a single `config.write()` (a `PUT /settings` or the config-file watcher's `apply_reloaded_config`) queues for the write lock. Every other `config.read().await` then blocks until VMAF finishes: settings/scan/system/stats handlers, notification `handle_event`, and other jobs' `config.read().await.clone()`. The whole server appears hung for the VMAF duration.
- **fix:** Snapshot the needed values and drop the guard before the long section: `let config = self.config.read().await.clone();` (owned snapshot, matching the pattern used elsewhere), or extract the few primitives into locals and `drop(config)` before line 2264.

---

## 🟠 Medium severity

### 7. Conversion jobs skip the zero-byte / size-integrity gate before promoting
- **category:** data-loss / error-handling
- **location:** `src/media/pipeline.rs:2235-2262` (and VMAF block 2265)
- **problem:** The output-validity gate is wrapped in `if !context.bypass_quality_gates && (output_size == 0 || …)`. For conversion jobs `bypass_quality_gates = true` (`pipeline.rs:1675`), so the whole block — including `output_size == 0` — is skipped, and the VMAF gate too. The only remaining net is the `delete_source` guard's `m.len() > 0` test at 2464.
- **impact:** A conversion where FFmpeg exits 0 but produces a 0-byte or truncated output (edge-case mux failure) is promoted over `output_path` and marked Completed. With `replace_strategy = replace`, a valid destination is replaced by the broken output. A non-zero-but-truncated output also passes the `len() > 0` delete_source guard → with `delete_source = true`, the source is deleted, losing the only good copy.
- **fix:** Always reject `output_size == 0` (plus a minimum plausibility check) regardless of `bypass_quality_gates`; keep only the compression-ratio/VMAF thresholds behind the bypass flag.

### 8. Spawned FFmpeg child not killed on future drop (orphan/leak)
- **category:** resource-leak
- **location:** `src/orchestrator.rs:273-275`, `src/media/ffmpeg/mod.rs:177,242` — `tokio::process::Command` built without `.kill_on_drop(true)`
- **problem:** The `Command` is never configured with `kill_on_drop(true)`. Cancellation is handled explicitly via `kill_rx`, but any path that drops the `run_ffmpeg_command` future without going through that channel leaves the child running — e.g. the byte-slice panic (#3) or a `JoinHandle::abort()`.
- **impact:** On task panic/abort the `ffmpeg` process is orphaned, keeps transcoding, keeps writing the temp file, consumes CPU/GPU, with no owner to reap it — leaked process plus an orphaned partial output.
- **fix:** Add `.kill_on_drop(true)` to the `Command` in `build()`, `extract_subtitles`, and wherever the transcode `Command` is constructed.

### 9. Subtitle-burn filtergraph escaping mishandles single quotes (injection)
- **category:** security / correctness
- **location:** `src/media/ffmpeg/mod.rs:483-486` and `503-508`
- **problem:** `render_filtergraph` builds `subtitles=filename='{}':si={idx}` and `escape_filter_path` does `.replace('\\',"\\\\").replace(':',"\\:").replace('\'',"\\'")`. Inside an FFmpeg single-quoted token a backslash is literal and the only terminator is `'`; a `'` cannot be escaped as `\'`. So a path with `'` closes the quote early and corrupts the filtergraph.
- **impact:** A file named `a'b.mkv` (legal) with subtitle burn-in produces `subtitles=filename='a\'b.mkv':si=0`, which FFmpeg mis-parses → job fails deterministically. A crafted name (`x':si=0,drawtext=… .mkv`) can inject additional filter options into the graph.
- **fix:** Use FFmpeg's real rule — replace `'` with `'\''` (close quote, escaped quote, reopen) — or pass the subtitle via a mapped input that needs no filtergraph escaping.

### 10. Missing index on `jobs.output_path` → quadratic library scans
- **category:** performance
- **location:** `src/db/jobs.rs:1285-1292`, called from `src/media/pipeline.rs:1287`
- **problem:** `has_job_with_output_path` runs `SELECT 1 FROM jobs WHERE output_path = ? AND archived = 0 LIMIT 1`. `output_path` is unindexed (only `input_path` is). `resolve_discovered_for_enqueue` calls it once per discovered file.
- **impact:** Every discovered file triggers a full table scan of `jobs`. For a scan of N files against M jobs this is O(N·M); on a large library the collision check dominates scan time, undermining the batched-insert optimization.
- **fix:** `CREATE INDEX IF NOT EXISTS idx_jobs_output_path ON jobs(output_path);` in a new additive migration.

### 11. Daily-boundary queries use wrong SQLite modifier order → mis-attributed "today" stats
- **category:** correctness
- **location:** `src/db/stats.rs:424-426,440,450,468,501`
- **problem:** They compare UTC-stored columns against `datetime('now','start of day','localtime')`. Modifiers apply left-to-right: `'now'` (UTC) → `'start of day'` (UTC midnight) → `'localtime'` (shift UTC-midnight by the local offset). The result is neither a correct UTC-day nor local-day boundary, while `updated_at`/`created_at` are stored as UTC.
- **impact:** In a non-UTC zone the "today" window is shifted by the UTC offset (e.g. at UTC+10 the boundary lands at 10:00 UTC). The daily-summary notification and Stats "today" lists report wrong counts near day boundaries.
- **fix:** For a UTC-day boundary drop `'localtime'`; for a local-day boundary use `datetime('now','localtime','start of day','utc')`. Be consistent.

### 12. Email (SMTP) notification target bypasses the SSRF / `allow_local` guard
- **category:** ssrf
- **location:** `src/notifications.rs:82-97` (`endpoint_url_for_target`), `908-913` / `1041-1050` (`send_email`)
- **problem:** `endpoint_url_for_target` returns `Ok(None)` for `"email"`, so `build_safe_client`'s private-IP / `allow_local_notifications` checks are skipped. `send_email` connects with `AsyncSmtpTransport::relay(&config.smtp_host)` using user-supplied host/port with no validation. Every HTTP target is DNS-resolved and blocked from private/loopback IPs unless `allow_local_notifications` is set; email is not.
- **impact:** A user restricted to configuring notification targets can set `smtp_host` to `169.254.169.254`, `127.0.0.1`, or any internal host + arbitrary port to probe the internal network / connect to internal SMTP relays, defeating the guard.
- **fix:** Apply the same resolve-and-reject-private-IP check to `smtp_host`/`smtp_port` (gated by `allow_local_notifications`) before building the SMTP transport.

### 13. Windows `\\?\` verbatim prefix breaks device-id grouping
- **category:** cross-platform
- **location:** `src/system/device_id.rs:14` (canonicalize) + `38-58` (`platform_device_id`)
- **problem:** `device_id_for` calls `std::fs::canonicalize`, which on Windows returns a verbatim path (`\\?\C:\…` or `\\?\UNC\server\share\…`). In `platform_device_id`, `chars.next()?` gets `\`, so the drive-letter branch is dead code; execution falls to `s.starts_with(r"\\")` (always true for `\\?\…`), parsed as UNC → `trimmed = &s[2..]` = `?\C:\…`, yielding `unc:?\C:`.
- **impact:** Source-drive grouping is wrong on Windows. Local drives happen to stay separated, but **every real UNC share collapses to the same id** (`\\?\UNC\serverA\share1` and `\\?\UNC\serverB\share2` both → `unc:?\UNC`), so the scheduler serializes jobs on different NAS boxes, defeating cross-device parallelism.
- **fix:** Strip the verbatim prefix before parsing, or match on `std::path::Component::Prefix` (`Disk`/`VerbatimDisk`/`UNC`/`VerbatimUNC`) instead of hand-parsing the string.

### 14. `classify_media_hint` amplifies each browse into O(N) recursive walks
- **category:** resource-leak / performance
- **location:** `src/system/fs_browser.rs:143` (per-entry call) and `519-548`
- **problem:** `browse_blocking` calls `classify_media_hint(&entry_path)` for every subdirectory entry. Each call runs `WalkDir::new(path).max_depth(2)…take(200)` — up to a depth-2 walk with ~200 syscalls. Browsing a directory with N subdirs performs up to N×200 filesystem operations synchronously inside one `spawn_blocking`.
- **impact:** Browsing a folder with thousands of children (common at a media root, or on a slow network mount) issues hundreds of thousands of syscalls and can block the browse request for many seconds to minutes, tying up a blocking thread.
- **fix:** Cap total work per browse (classify only the first K entries, or make the hint lazy/on-demand), reduce `max_depth`/`take`, or short-circuit the cheap name-based `MediaHint::High` check before the walk and skip the walk when there are many entries.

### 15. Lost-update race in config read-modify-write settings handlers
- **category:** race
- **location:** `src/server/settings.rs:82` (and `153,282,787,847,900,1089`; `scan.rs:616,668,714`; `jobs.rs:1050`)
- **problem:** Every settings mutation is `let mut next = state.config.read().await.clone();` → mutate → `save_config_or_response(...)` (disk write) → then a separate `let mut config = state.config.write().await; *config = next;`. No lock spans read→save→write.
- **impact:** Two concurrent settings requests both clone the same baseline, each applies only its own fields, both persist; the last writer's full snapshot wins — silently discarding the other request's change in memory and in `config.toml`.
- **fix:** Serialize the whole read-modify-persist-write with a dedicated `tokio::sync::Mutex` (config-update lock) held across the handler body, or mutate under a single `config.write().await` guard and persist while holding it.

---

## 🟡 Low severity

### 16. Cancel during finalize mislabels a completed job as Cancelled while still deleting source
- **category:** correctness / state-consistency
- **location:** `src/media/pipeline.rs:2130-2159`, `2416-2489`
- **problem:** `finalize_job` promotes the temp file (2416) then calls `update_job_state(Completed)` (2418). If a cancel was requested during finalize, `update_job_state` intercepts and writes `Cancelled`, returning `Ok(())`. Because it returns Ok, `?` succeeds and finalize continues to record a completed encode attempt, emit success telemetry, and run `delete_source`.
- **impact:** Output validly promoted, but the job is recorded as `Cancelled` while telemetry/encode-attempt say completed and the source is deleted. No data loss, but inconsistent state.
- **fix:** Past `promote_temp_artifact`, commit `Completed` unconditionally, or detect the intercept and short-circuit the completion bookkeeping consistently.

### 17. Windows output promotion is non-atomic (remove-then-rename)
- **category:** data-loss (Windows)
- **location:** `src/media/pipeline.rs:2199-2205` (`promote_temp_artifact`)
- **problem:** `if cfg!(windows) && final_path.exists() { std::fs::remove_file(final_path)?; } std::fs::rename(temp_path, final_path)?;` — on Windows the destination is deleted before the rename, leaving an empty slot in between.
- **impact:** On Windows a crash/power loss between `remove_file` and `rename` leaves the destination gone with only `*.alchemist.tmp` present. Under `replace_strategy = replace` the pre-existing destination is lost. POSIX is unaffected (single atomic `rename`).
- **fix:** Rename destination to a backup, rename temp into place, then delete the backup — or use `MoveFileEx`/`ReplaceFileW` with `MOVEFILE_REPLACE_EXISTING`.

### 18. Paths converted with `display().to_string()` corrupt non-UTF-8 filenames
- **category:** correctness
- **location:** `src/media/ffmpeg/mod.rs:188-194,209,326,343,353`
- **problem:** Every path passed to FFmpeg goes through `self.input.display().to_string()`. `Path::display()` is lossy: invalid UTF-8 bytes (legal on Linux) become U+FFFD. `build_args` returns `Vec<String>`, forcing lossy conversion even though `Command::arg` accepts `OsStr`.
- **impact:** A file with non-UTF-8 bytes yields a mangled path; FFmpeg can't open it → deterministic failure (no data loss, but the file can never be processed). Rare but real on Linux.
- **fix:** Pass `OsStr` paths straight to `Command::arg` for input/output operands (keep the `Vec<String>` builder for testable flag ordering, append the real `OsStr` path in `build()`).

### 19. NVENC VBR+CQ without `-b:v 0` can cap quality
- **category:** correctness
- **location:** `src/media/ffmpeg/nvenc.rs:16-51`
- **problem:** Each NVENC branch emits `-rc vbr -cq <n>` but never `-b:v 0` (`mod.rs` only adds `-b:v` for `Bitrate` rate control). NVENC in `vbr` with a nonzero default target bitrate constrains output rather than honoring CQ as a pure quality knob.
- **impact:** HEVC/H264/AV1 NVENC encodes may be bitrate-limited instead of quality-targeted, producing inconsistent quality versus the CPU CRF path.
- **fix:** Add `-b:v 0` alongside `-cq` in the NVENC branches.

### 20. QSV `-look_ahead 20` is a boolean flag, not the depth
- **category:** correctness
- **location:** `src/media/ffmpeg/qsv.rs:33-36,44-46,53-56`
- **problem:** For QSV `-look_ahead` is boolean (0/1); the depth is `-look_ahead_depth`. Passing `20` enables lookahead but doesn't set depth 20; for `av1_qsv` `-look_ahead` may not exist, causing an encoder-open error.
- **impact:** The intended 20-frame lookahead depth is silently not applied on H264/HEVC QSV; on AV1 QSV the encode may fail to open and consistently fall back to CPU.
- **fix:** Use `-look_ahead 1 -look_ahead_depth 20`, gated to encoders that support it (verify against `av1_qsv`).

### 21. `VACUUM INTO` interpolates the backup path directly into SQL
- **category:** correctness
- **location:** `src/db/system.rs:358-364` (`create_online_backup`)
- **problem:** `sqlx::query(&format!("VACUUM INTO '{}'", path_str))` interpolates the path (`VACUUM INTO` can't bind a parameter, but single quotes must be doubled). A path with `'` (e.g. `/home/o'brien/backup.db`) produces malformed SQL.
- **impact:** Online backup silently fails/errors for any destination path containing `'`; the user gets no valid backup.
- **fix:** Double single quotes before interpolating: `path_str.replace('\'', "''")`.

### 22. Missing index on `logs.job_id`
- **category:** performance
- **location:** `src/db/system.rs:59-72` (`get_logs_for_job`)
- **problem:** `SELECT … FROM logs WHERE job_id = ? ORDER BY created_at … LIMIT ?`. `logs` has only `idx_logs_created_at`; `job_id` is unindexed.
- **impact:** Per-job log retrieval (job detail view, completion paths) scans the entire `logs` table filtered by `job_id`; slow on a busy instance with a large log table.
- **fix:** `CREATE INDEX IF NOT EXISTS idx_logs_job_id ON logs(job_id);` in a new additive migration.

### 23. Login endpoint leaks raw DB error strings to unauthenticated callers
- **category:** security
- **location:** `src/server/auth.rs:46-52`
- **problem:** On a user-lookup DB failure the handler returns `api_error_response(500, "AUTH_LOOKUP_FAILED", err.to_string())`, placing the raw error in the JSON `detail`. Reachable pre-authentication.
- **impact:** An unauthenticated caller can provoke DB errors and receive internal details (sqlx/SQLite messages, possibly schema/path fragments), aiding recon.
- **fix:** Log `err` server-side (already done) and return a generic detail without `err.to_string()`.

### 24. Setup token compared non-constant-time and not URL-decoded
- **category:** auth
- **location:** `src/server/middleware.rs:118-122`
- **problem:** The `ALCHEMIST_SETUP_TOKEN` check does a plain `==` (not constant-time) on the raw, un-URL-decoded query value.
- **impact:** Timing side-channel is not practically exploitable for a high-entropy token gating only the first-run window (low), but the URL-decode gap is a correctness footgun (a valid token with special chars silently fails).
- **fix:** Compare using a constant-time comparison (e.g. `subtle`, consistent with session/api-token hashing) and percent-decode the query value first.

### 25. `redact.rs` webhook masking uses `rfind('/')`, leaking Discord `/slack` `/github` token
- **category:** secret-leak
- **location:** `src/redact.rs:132-150`
- **problem:** `redact_webhook_urls` masks only the segment after the last slash. Discord exposes `.../api/webhooks/{id}/{token}/slack` and `/github`; for those `rfind('/')` points at the slash before `slack`/`github`, so only that word is masked and the `{token}` stays cleartext. Telegram `bot{token}` paths aren't targeted at all.
- **impact:** A Discord slack/github webhook URL reaching the (redacted) DB log store still exposes its token — a gap between what redact.rs claims and does.
- **fix:** Anchor the mask to the known prefix (mask everything after `/api/webhooks/{id}/`, stop at the next `/` or `?`); add Telegram `bot{token}` handling if in scope.

### 26. Schedule window with `start_time == end_time` silently never activates
- **category:** correctness
- **location:** `src/scheduler.rs:99-107`; `src/config.rs:1107-1115` (validate)
- **problem:** The same-day branch gates on `current >= start && current < end`. When `start == end` this is never true, so the window is always inactive and the engine stays RESTRICTED. Config validation never rejects `start == end` (unlike quiet hours, which does).
- **impact:** A user setting an all-day `00:00`–`00:00` window (natural "24h" expectation) gets an engine that never runs during it, with no error at save time.
- **fix:** Reject `start_time == end_time` in `Config::validate` for schedule windows (mirroring quiet hours), or treat equal start/end as a full-day window explicitly.

### 27. Disk guardrail fails open for verbatim/mismatched-prefix output paths
- **category:** cross-platform
- **location:** `src/system/disk_space.rs:26` (`path.starts_with(mount)`)
- **problem:** Mount matching uses `path.starts_with(mount)`, which is prefix-kind sensitive. `sysinfo` reports Windows mounts as `C:\` (`Prefix::Disk`); a canonicalized/verbatim output path (`\\?\C:\…`, `Prefix::VerbatimDisk`) fails to match, so no mount is found and the function returns `None`.
- **impact:** `is_below_min_free(None, …)` fails open (returns false), so the free-space guardrail is silently skipped for any output path whose prefix kind doesn't match the mount — the engine may start an encode that fills the disk. Same `\\?\` root cause as #13.
- **fix:** Normalize both `path` and `mount` to the same prefix representation before comparing, or compare by resolved volume/root.

### 28. `fs_browser` sensitive-path blocklist is advisory / incomplete
- **category:** security
- **location:** `src/system/fs_browser.rs:336-380` (`is_sensitive_path`)
- **problem:** The module intentionally browses the whole filesystem (server-side directory picker); the only confinement is the `is_sensitive_path` denylist. On Unix it blocks a fixed prefix set (`/etc`, `/root`, `/proc`, …) but not e.g. `/home/*/.ssh`, `/var/lib`, user dotfiles; on Windows only four substrings. No allow-root model. (Note: exposes directory/entry *names* only — never file contents — and symlink escapes are correctly blocked after canonicalization.)
- **impact:** An authenticated caller can enumerate directory structure outside any media root (e.g. list `/home/otheruser`). Name disclosure only, for an admin-facing tool → low, but the guard is a denylist, not a boundary.
- **fix:** Switch to an allow-list of configured library roots + whitelisted mounts, or extend the denylist (`.ssh`, dotfiles, `/var`) and document that full-filesystem enumeration is by design + admin-gated.

---

## Verified clean

- **Frontend (`web/src/**`):** no XSS sinks (`dangerouslySetInnerHTML`/`set:html`/`innerHTML` = zero hits; all user/media strings rendered as escaped JSX text), SSE hooks (`useJobSSE.ts`, `LogViewer.tsx`, `HardwareSettings.tsx`) close/clear on cleanup with capped backoff, optimistic writes (saved views) roll back on failure and SSE patches are reconciled with real refetches.
- **Migrations:** fully additive-only — no `DROP`/`RENAME`/type-change; only `ADD COLUMN` / `CREATE … IF NOT EXISTS`. v0.2.5 baseline + `integration_db_upgrade.rs` remain safe.
- **`.unwrap()`/`.expect()`:** none in production paths across all audited backend files (only `unwrap_or*` and test-only `panic!`).
- **Updater integrity:** streams download, hashes on the fly, rejects on SHA-256 mismatch against an ed25519-signature-verified manifest; install gated on `Verified`; argv (no shell) helper; same-filesystem atomic swap with rollback; HTTPS fetches. (Minor residual: extracted binary re-checked via `--version`, not re-hashed — local TOCTOU in a 64-bit-random temp dir, single-tenant.)
- **HTTP-target SSRF:** `build_safe_client` resolves DNS, pins the connection (`.resolve()`, blocks rebinding), disables redirects, blocks private/loopback/link-local/multicast incl. IPv4-mapped IPv6. (Email is the gap — see #12.)
- **Auth coverage:** every `/api` route behind `auth_middleware`; `/api/v1/*` normalized so allowlists apply uniformly; public set limited to setup/login/logout/health/ready; no state-changing route bypasses auth. Sessions/API-tokens SHA-256-hashed before indexed lookup (timing-safe); Argon2 login uses dummy-hash + `spawn_blocking` to equalize timing.
- **CSRF:** session cookie `HttpOnly; SameSite=Lax`; all state-changing endpoints are POST/PUT/PATCH/DELETE.
- **Telemetry:** fixed-literal failure reasons, gated on `enable_telemetry`, HTTPS.
- **MCP server:** read-only, no mutation tools, bounds-checked `limit`.
- **Orchestrator `std::sync::Mutex` across `.await`:** the "never held across await" claim holds within `run_ffmpeg_command` (the separate defect is cross-function lock *ordering* — #2). Broadcast `RecvError::Lagged`/`Closed` handled correctly in sse/notifications/metrics.
