# Audit Findings

Last updated: 2026-04-23

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

---

### [RG-6] Cancelled backup downloads leave full SQLite snapshots behind in the temp directory

**Status: RESOLVED**

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

1. **[TD]** Added unit tests for the segment concatenation logic in `src/media/pipeline.rs`.
2. **[UX]** Implemented a "Clear tab" button in the Jobs table for the terminal tabs (Failed, Cancelled, Completed) and added a dedicated "Cancelled" tab.
3. **[FG]** Added support for custom FFmpeg filter strings in the Library Profile settings.
