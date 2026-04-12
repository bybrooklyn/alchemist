# Audit Findings

Date: 2026-04-11

## Summary

This audit focused on the highest-risk paths in Alchemist:

- queue claiming and cancellation
- media planning and execution
- conversion validation
- setup/auth exposure
- job detail and failure UX

The current automated checks were green at audit time, but several real
correctness and behavior issues remain.

## Findings

### [P1] Canceling a job during analysis can be overwritten

Relevant code:

- `src/server/jobs.rs:41`
- `src/media/pipeline.rs:927`
- `src/media/pipeline.rs:970`
- `src/orchestrator.rs:239`

`request_job_cancel()` marks `analyzing` and `resuming` jobs as
`cancelled` immediately. But the analysis/planning path can still run to
completion and later overwrite that state to `skipped`,
`encoding`/`remuxing`, or another follow-on state.

The transcoder-side `pending_cancels` check only applies around FFmpeg
spawn, so a cancel issued during analysis is not guaranteed to stop the
pipeline before state transitions are persisted.

Impact:

- a user-visible cancel can be lost
- the UI can report a cancelled job that later resumes or becomes skipped
- queue state becomes harder to trust

### [P1] VideoToolbox quality controls are effectively a no-op

Relevant code:

- `src/config.rs:85`
- `src/media/planner.rs:633`
- `src/media/ffmpeg/videotoolbox.rs:3`
- `src/conversion.rs:424`

The config still defines a VideoToolbox quality ladder, and the planner
still emits `RateControl::Cq` for VideoToolbox encoders. But the actual
VideoToolbox FFmpeg builder ignores rate-control input entirely.

The Convert workflow does the same thing by still generating `Cq` for
non-CPU/QSV encoders even though the VideoToolbox path does not consume
it.

Impact:

- quality profile does not meaningfully affect VideoToolbox jobs
- Convert quality values for VideoToolbox are misleading
- macOS throughput/quality tradeoffs are harder to reason about

### [P2] Convert does not reuse subtitle/container compatibility checks

Relevant code:

- `src/media/planner.rs:863`
- `src/media/planner.rs:904`
- `src/conversion.rs:272`
- `src/conversion.rs:366`

The main library planner explicitly rejects unsafe subtitle-copy
combinations, especially for MP4/MOV targets. The Convert flow has its
own normalization/build path and does not reuse that validation.

Impact:

- the Convert UI can accept settings that are known to fail later in FFmpeg
- conversion behavior diverges from library-job behavior
- users can hit avoidable execution-time errors instead of fast validation

### [P2] Completed job details omit metadata at the API layer

Relevant code:

- `src/server/jobs.rs:344`
- `web/src/components/JobManager.tsx:1774`

The job detail endpoint explicitly returns `metadata = None` for
`completed` jobs, even though the Jobs modal is structured to display
input metadata when available.

Impact:

- completed-job details are structurally incomplete
- the frontend needs special-case empty-state behavior
- operator confidence is lower when comparing completed jobs after the fact

### [P2] LAN-only setup is easy to misconfigure behind a local reverse proxy

Relevant code:

- `src/server/middleware.rs:269`
- `src/server/middleware.rs:300`

The setup gate uses `request_ip()` and trusts forwarded headers only when
the direct peer is local/private. If Alchemist sits behind a loopback or
LAN reverse proxy that fails to forward the real client IP, the request
falls back to the proxy peer IP and is treated as LAN-local.

Impact:

- public reverse-proxy deployments can accidentally expose setup
- behavior depends on correct proxy header forwarding
- the security model is sound in principle but fragile in deployment

## What To Fix First

1. Fix the cancel-during-analysis race.
2. Fix or redesign VideoToolbox quality handling so the UI and planner do
   not promise controls that the backend ignores.
3. Reuse planner validation in Convert for subtitle/container safety.
4. Decide whether completed jobs should persist and return metadata in the
   detail API.

## What To Investigate Next

1. Use runtime diagnostics to confirm whether macOS slowness is true
   hardware underperformance, silent fallback, or filter overhead.
2. Verify whether “only one job at a time” is caused by actual worker
   serialization or by planner eligibility/skips.
3. Review dominant skip reasons before relaxing planner heuristics.
