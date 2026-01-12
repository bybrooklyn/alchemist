# Design Philosophy

This document defines the principles that govern every major and minor part of the Alchemist project.
It is meant to keep the system stable, coherent, and forward compatible over time.

## 1) Product Intent
- Alchemist is a reliability-first media pipeline.
- The system favors predictability and correctness over novelty.
- Every feature should be operable by non-experts without removing power from experts.

## 2) Stability Over Novelty
- Do not introduce breaking changes unless there is no alternative.
- When in doubt, add new capabilities without removing old ones.
- Fail safe; avoid data loss as the default outcome.

## 3) Backwards and Forwards Compatibility
- Databases created on v0.2.5+ must remain usable for all future versions.
- New code must read old data without requiring manual migration steps.
- Schema changes should be additive only:
  - Add columns with defaults or nullable values.
  - Add new tables rather than mutate or drop old ones.
  - Never rename or remove columns.
- Compatibility logic in code must tolerate missing fields and legacy table shapes.

## 4) Reliability and Observability
- Favor deterministic behavior over clever heuristics.
- Every long-running process should be monitorable and cancellable.
- Log critical transitions and errors with actionable context.

## 5) Safety and Data Integrity
- Never overwrite user media by default.
- Always prefer reversible actions.
- Validate inputs at boundaries (API, CLI, filesystem).
- Defensive programming: assume file states can change at any time.

## 6) Performance and Scale
- Optimize for large libraries and long runtimes.
- Prefer bounded memory usage over raw speed.
- Use indexes and incremental scans for large datasets.
- Avoid unnecessary reprocessing or re-probing of files.

## 7) Security and Privacy
- Authentication and authorization are mandatory for protected APIs.
- Use secure defaults for tokens and cryptography.
- Telemetry must be opt-in, minimal, and anonymized.

## 8) Configuration Is a Contract
- Config changes must be validated and safe to apply live.
- Defaults should be safe and conservative.
- Every config option must have a clear and visible purpose.

## 9) UI and UX Consistency
- UI must reflect backend truth; avoid optimistic UI unless reconciled.
- Never hide errors; show the user what failed and why.
- UI should be fast, responsive, and readable on small screens.

## 10) Cross-Platform Discipline
- All core features must work on macOS, Linux, and Windows unless explicitly documented.
- Build pipelines must be deterministic and repeatable on CI and developer machines.

## 11) Incremental Architecture
- Prefer small, composable modules.
- Avoid tight coupling between UI and core pipeline logic.
- Stable APIs and event streams are more important than rapid refactors.

## 12) Testing and Verification
- Test the critical paths: scan, enqueue, analyze, encode, finalize.
- Every migration should be tested against a v0.2.5+ baseline DB.
- Tests must be deterministic and reproducible.

## 13) Documentation and Traceability
- Document behavior changes alongside code changes.
- Keep release notes aligned with schema evolution.
- Every new feature must include an explanation of its operational impact.

## 14) Maintenance and Lifecycle
- Add cleanup tasks for long-lived data (logs, sessions, temp files).
- Make maintenance tasks visible and safe to run.
- Avoid silent failures; surface and recover wherever possible.

## 15) Decision-Making Rules
- If a change risks data loss, do not merge it.
- If a change risks breaking older data, redesign it.
- If a change simplifies code but harms clarity or reliability, reject it.

## 16) Style and Engineering Practices
- Keep code explicit and readable; avoid cleverness.
- Keep functions small and well-named.
- Prefer explicit error handling over implicit fallbacks.

---

This philosophy is binding unless explicitly overridden in a documented exception.
