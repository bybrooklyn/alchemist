---
name: audit
description: Audits the Alchemist codebase for bugs, security issues, performance problems, and correctness gaps. Writes findings to audit.md with severity ratings and detailed fix instructions. Run with /audit to do a full sweep, or /audit <area> to focus on a subsystem (e.g. /audit db, /audit frontend, /audit notifications).
---

# Codebase Audit

Perform a systematic audit of the Alchemist codebase. Find real bugs, broken logic, security issues, and performance problems. Write every new finding to `audit.md` with severity and step-by-step fix instructions.

## Phase 1: Read Existing Audit

**Always start here.** Read `audit.md` in full before doing anything else. Extract every existing issue ID (P1-1, P2-2, TD-3, etc.) and their titles. You must not write duplicate entries — if a problem is already documented, skip it.

Also read `CLAUDE.md` to understand the architecture, constraints, and key design rules (no `.unwrap()`, additive-only schema changes, no data loss, etc.).

## Phase 2: Scope

If the user passed an argument (e.g. `/audit db`), focus only on files for that subsystem. Otherwise audit everything.

**Full audit file list:**

Backend:
- `src/db.rs` — all queries, migrations, schema
- `src/media/pipeline.rs` — encode flow, state transitions, finalize logic
- `src/media/executor.rs` — FFmpeg execution
- `src/media/planner.rs` — transcode decision logic
- `src/media/analyzer.rs` — ffprobe wrapper
- `src/media/ffmpeg/mod.rs` + submodules (`videotoolbox.rs`, `nvenc.rs`, `qsv.rs`, `vaapi.rs`, `amf.rs`, `cpu.rs`)
- `src/server/jobs.rs` — job API handlers
- `src/server/scan.rs` — library scan handlers
- `src/server/system.rs` — hardware/library intelligence handlers
- `src/server/settings.rs` — config read/write handlers
- `src/server/stats.rs` — stats/savings handlers
- `src/server/auth.rs` — login, session management
- `src/server/middleware.rs` — rate limiting, auth middleware
- `src/server/sse.rs` — server-sent events
- `src/server/wizard.rs` — setup wizard handlers
- `src/orchestrator.rs` — FFmpeg process spawning
- `src/config.rs` — config structs
- `src/scheduler.rs` — off-peak cron scheduling
- `src/notifications.rs` — Discord, Gotify, Webhook
- `src/error.rs` — error types
- `migrations/` — all SQL migration files

Frontend:
- `web/src/components/JobManager.tsx`
- `web/src/components/jobs/useJobSSE.ts`
- `web/src/components/jobs/JobDetailModal.tsx`
- `web/src/components/jobs/JobsTable.tsx`
- `web/src/components/jobs/JobsToolbar.tsx`
- `web/src/components/jobs/types.ts`
- `web/src/components/SettingsPanel.tsx` (if exists)
- `web/src/components/Dashboard.tsx` (if exists)

## Phase 3: What To Look For

For each file, look for:

### Bugs & Logic Errors
- Incorrect error handling (returning `Ok` when should return `Err`, swallowing errors with `let _ =`)
- State machine violations (job transitions that skip states or allow invalid transitions)
- Race conditions and TOCTOU patterns (check-then-act with lock dropped between check and act)
- Off-by-one errors, wrong comparison operators
- Integer overflow/underflow (subtraction on unsigned or signed types without saturation)
- Silent failures (`.ok()`, `.unwrap_or_default()` hiding real errors)
- Dead code paths that never execute but look like they should

### Security Issues
- SQL injection surface (user input in query strings, not just bound params)
- LIKE wildcard injection (`_` and `%` not escaped in LIKE patterns)
- Path traversal (user-supplied paths not validated against allowed roots)
- Auth bypass (endpoints reachable without auth middleware)
- Session token leakage (tokens in logs, error messages)
- Missing authorization checks (checking authentication but not authorization)

### Performance Problems
- N+1 query patterns (query inside a loop)
- Unbounded queries (no LIMIT on queries that could return the full table)
- Blocking async (CPU-heavy work or blocking I/O on the async executor without `spawn_blocking`)
- Unnecessary clones of large data structures
- Per-call schema introspection (PRAGMA queries that should be cached)
- Missing database indexes on frequently-filtered columns
- Spawning subprocesses (ffprobe, ffmpeg) in response to HTTP requests without rate limiting

### Correctness Issues
- Fields that are always zeroed/empty but structurally appear meaningful
- Config values that map to the wrong FFmpeg flag (inverted scales, wrong units)
- Stale UI state (frontend not updated when backend changes)
- SSE events that can overwrite newer data with older data
- Struct fields returned in API responses that are never populated

### Frontend Issues
- React state update after unmount
- `useEffect` with stale closure over mutable values
- Optimistic UI that isn't reconciled with server truth
- Missing loading/error states for async operations
- Type `any` or unchecked casts masking real type errors

### Database / Migration Issues
- Schema changes that rename or drop columns (violates backwards compat rule)
- Missing `AND archived = 0` filter on queries that should exclude soft-deleted rows
- Missing `COALESCE` on nullable columns mapped to non-optional Rust fields
- Queries without indexes that will scan full table at scale

## Phase 4: Severity Classification

Assign each finding a severity tier:

| Tier | Label | Criteria |
|------|-------|----------|
| P1 | Critical | Data loss, data corruption, security bypass, user-visible correctness bug (wrong output, wrong state) |
| P2 | High | Feature partially broken, silent failure, significant performance problem in hot path |
| TD | Technical Debt | Code quality, maintainability, structural problem — no user-visible impact today |
| RG | Reliability Gap | Works now but fragile under load, partial failure, or restart |
| UX | UX Gap | Feature works but is confusing, missing feedback, or harder to use than it should be |
| FG | Feature Gap | Missing capability users would reasonably expect |

## Phase 5: Write to audit.md

### Rules

1. **Read audit.md first.** Find the highest existing ID in each tier (e.g. if P2-3 exists, the next P2 is P2-4).
2. **No duplicates.** If the same root problem already has an entry, skip it.
3. **Append to the correct section.** Each tier has its own `## P1 Issues`, `## P2 Issues`, etc. section. Add new entries at the bottom of the relevant section.
4. **If a section doesn't exist yet**, create it with the correct `## Heading`.
5. **Update "Last updated" date** at the top of audit.md.
6. **Update "What To Fix First"** at the bottom if any P1s were added.

### Entry Format

Every entry must follow this exact structure:

```markdown
### [TIER-N] Short title describing the problem

**Files:**
- `path/to/file.rs:line_start–line_end` — one-line description of what's here
- `path/to/other.rs:line` — one-line description

**Severity:** P1 / P2 / TD / RG / UX / FG

**Problem:**

2–5 sentence description of what is wrong. Be specific: name the function, the variable,
the condition. Show the problematic code snippet if it makes the problem clearer.

```rust
// bad code here showing the issue
```

**Fix:**

Numbered steps. Each step should be a concrete code change, not vague advice.
Include exact function names, field names, and SQL. If there are options, explain the
tradeoff and recommend one.

1. In `path/to/file.rs`, change X to Y:
   ```rust
   // before
   old_code();
   // after
   new_code();
   ```
2. In `path/to/other.rs`, add validation:
   ```rust
   if condition { return Err(...); }
   ```
3. Add a test in `tests/` that exercises the fixed path.
```

## Phase 6: Summary Report

After writing to audit.md, report back to the user:

1. How many new issues were found, broken down by tier
2. The IDs and one-line descriptions of each new finding
3. Which existing issues (if any) were already resolved (code no longer matches the documented problem)
4. What to fix first among the new findings

If you find that an existing audit.md entry is **already fixed** (the code no longer matches the documented problem), add a `**Status: RESOLVED**` line to that entry in audit.md and note it in your summary.

## Focused Audit Arguments

If the user runs `/audit <subsystem>`, map to these file sets:

| Argument | Files |
|----------|-------|
| `db` | `src/db.rs`, `migrations/` |
| `pipeline` | `src/media/pipeline.rs`, `src/media/executor.rs`, `src/media/planner.rs`, `src/media/analyzer.rs` |
| `ffmpeg` | `src/media/ffmpeg/` (all files) |
| `server` | `src/server/` (all files) |
| `frontend` | `web/src/components/` (all files) |
| `notifications` | `src/notifications.rs`, `src/config.rs` (notification structs only) |
| `auth` | `src/server/auth.rs`, `src/server/middleware.rs`, `src/db.rs` (session queries only) |
| `config` | `src/config.rs`, `src/server/settings.rs` |

## Constraints (from CLAUDE.md)

These are binding rules — violations are always at least P1:
- Never `.unwrap()` or `.expect()` in production code paths (clippy enforces this, but audit catches logic-level misuse of `.ok()` and `.unwrap_or_default()`)
- Schema changes must be additive only — no column renames or drops
- Never overwrite user media by default
- All core features must work on macOS, Linux, and Windows
- Databases from v0.2.5+ must remain usable
