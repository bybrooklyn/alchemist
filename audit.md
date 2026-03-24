# Alchemist UI Rework — Prompt 2 Audit Report

**Date:** 2026-03-24  
**Status:** ✅ All checks pass

---

## Verification Matrix

| # | Check | File(s) | Result |
|---|-------|---------|--------|
| 1 | `setup.astro` uses `app-shell` + `SetupSidebar` | `setup.astro` | ✅ |
| 2 | `SetupSidebar.astro` exists with grayed nav + footer | `SetupSidebar.astro` | ✅ |
| 3 | `SetupFrame.tsx` has 2px progress bar | `SetupFrame.tsx:29-38` | ✅ |
| 4 | Error triggers `showToast` via `useEffect` | `SetupFrame.tsx:19-23` | ✅ |
| 5 | Step 5 button reads "Complete Setup" (not "Build Engine") | `SetupFrame.tsx:94-95` | ✅ |
| 6 | Navigation footer with step counter | `SetupFrame.tsx:59-102` | ✅ |
| 7 | `LibraryStep` is single-column (no side-by-side) | `LibraryStep.tsx` | ✅ |
| 8 | No preview panel in `LibraryStep` | `LibraryStep.tsx` | ✅ |
| 9 | Recommendations as flat list with Add/Added | `LibraryStep.tsx:112-168` | ✅ |
| 10 | Selected folders as chips with X | `LibraryStep.tsx:185-223` | ✅ |
| 11 | Browse button + manual path input | `LibraryStep.tsx:225-273` | ✅ |
| 12 | `ReviewCard` title — no `uppercase tracking-wide` | `SetupControls.tsx:96` | ✅ |
| 13 | `ScanStep` — no `text-[10px]` | `ScanStep.tsx` | ✅ |
| 14 | `ScanStep` — no `tracking-widest` | `ScanStep.tsx` | ✅ |
| 15 | `ScanStep` — no `rounded-xl` | `ScanStep.tsx` | ✅ |
| 16 | `ProcessingStep` — no `text-[10px]` | `ProcessingStep.tsx:45` | ✅ |
| 17 | `JobManager` stat cards → inline summary | `JobManager.tsx:557-576` | ✅ |
| 18 | `JobManager` status badges: `capitalize`, no `tracking` | `JobManager.tsx:531` | ✅ |
| 19 | `JobManager` table header: no `uppercase tracking-wider` | `JobManager.tsx:711` | ✅ |
| 20 | `JobManager` — zero `rounded-xl` remaining | grep | ✅ |
| 21 | `JobManager` — zero `uppercase tracking` remaining | grep | ✅ |
| 22 | `SystemSettings` has `EngineMode` + `EngineStatus` interfaces | `SystemSettings.tsx:13-27` | ✅ |
| 23 | `SystemSettings` fetches `/api/engine/mode` + `/api/engine/status` | `SystemSettings.tsx:43-56` | ✅ |
| 24 | `SystemSettings` has `handleModeChange` handler | `SystemSettings.tsx:94-125` | ✅ |
| 25 | `SystemSettings` renders mode buttons + computed limits | `SystemSettings.tsx:138-208` | ✅ |
| 26 | `HeaderActions` — no `EngineMode` interface | grep | ✅ |
| 27 | `HeaderActions` — no `engineMode` state | grep | ✅ |
| 28 | `HeaderActions` — no `refreshEngineMode` | grep | ✅ |
| 29 | `HeaderActions` — no `handleModeChange` | grep | ✅ |
| 30 | `HeaderActions` — no `handleApplyAdvanced` | grep | ✅ |
| 31 | `HeaderActions` — no `showAdvanced` / `manualJobs` / `manualThreads` | grep | ✅ |

---

## Banned Pattern Sweep (modified files only)

| Pattern | Occurrences |
|---------|-------------|
| `uppercase tracking` | 0 |
| `tracking-wide` | 0 |
| `tracking-wider` | 0 |
| `tracking-widest` | 0 |
| `text-[10px]` | 0 |
| `text-[11px]` | 0 |
| `rounded-xl` | 0 |
| `rounded-2xl` | 0 |
| `bg-clip-text` | 0 |
| `text-transparent` | 0 |
| `Build Engine` | 0 |

> [!NOTE]
> Banned patterns **do** appear in files NOT in scope (e.g. `TranscodeSettings.tsx`, `HardwareSettings.tsx`, `WatchFolders.tsx`). These were not listed for modification in the prompt.

---

## TypeCheck

```
$ bun run typecheck
$ tsc -p tsconfig.json --noEmit
(exit 0 — zero errors)
```

---

## Files Modified

| File | Action |
|------|--------|
| `web/src/pages/setup.astro` | Rewritten |
| `web/src/components/SetupSidebar.astro` | **New** |
| `web/src/components/setup/SetupFrame.tsx` | Rewritten |
| `web/src/components/setup/LibraryStep.tsx` | Rewritten |
| `web/src/components/setup/SetupControls.tsx` | Patched (1 line) |
| `web/src/components/setup/ScanStep.tsx` | Patched (4 sites) |
| `web/src/components/setup/ProcessingStep.tsx` | Patched (1 line) |
| `web/src/components/JobManager.tsx` | Patched (17 sites) |
| `web/src/components/SystemSettings.tsx` | Rewritten |
| `web/src/components/HeaderActions.tsx` | Rewritten |

---

## Additional Runtime Hardening

These changes were merged from the `claude/distracted-kalam` worktree while resolving Git state into `master`.

| File | Change |
|------|--------|
| `justfile` | Safer `dev` process cleanup, stronger DB reset cleanup (`-wal` / `-shm`), `find`/`xargs` safety fixes, and frozen-lockfile docs install |
| `src/media/analyzer.rs` | Moved FFprobe execution to `tokio::process::Command` with a 120s timeout helper |
| `src/notifications.rs` | Warn on invalid notification event JSON instead of silently disabling targets |
| `src/orchestrator.rs` | Recover from poisoned cancellation locks and truncate oversized FFmpeg stderr lines |
| `src/scheduler.rs` | Warn on invalid schedule day JSON instead of silently treating it as empty |
