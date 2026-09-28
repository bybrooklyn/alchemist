# Alchemist — Ideas

*Forward-looking ideas for features, UX, integrations, and polish. Bugs go in `audit.md`.*

**Last updated:** 2026-09-03

## Inbox

<!-- BEGIN GENERATED ideas index — do not edit inside this region; add thoughts under the Inbox heading -->
<!-- generated-hash: 3fdfca937758275742e6e7d7edc597b32ca43b94ab790d356617befefc79f3e4 -->

### Seed

- [Automation](.fractalcode/ideas/ms3j9g5l10-automation.md)
- [Encoding](.fractalcode/ideas/ms3j9g5l9-encoding.md)
- [Features](.fractalcode/ideas/ms3j9g5l2-features.md)
- [Integrations](.fractalcode/ideas/ms3j9g5l4-integrations.md)
- [Migration and Improvements](.fractalcode/ideas/ms3j9g5l11-migration-and-improvements.md)
- [Observability](.fractalcode/ideas/ms3j9g5l7-observability.md)
- [Operator](.fractalcode/ideas/ms3j9g5l8-operator.md)
- [Performance](.fractalcode/ideas/ms3j9g5l5-performance.md)
- [Polish](.fractalcode/ideas/ms3j9g5l6-polish.md)
- [Top picks](.fractalcode/ideas/ms3j9g5l-top-picks.md)
- [UX](.fractalcode/ideas/ms3j9g5l3-ux.md)
<!-- END GENERATED -->

## Migration and Improvements

### [IMPR-3] Design `libalchemist` as a stable media-planning engine

**Category:** Improvement
**Size:** L
**Touches:** backend, architecture, tests, docs

**Problem or gap:**

The package already emits a Rust library named `alchemist`, but `src/lib.rs`
publishes nearly the entire application and therefore exposes server, database,
authentication, update, and process-lifecycle internals as if they were a
supported API. Core planning is also coupled to persistence through
`db::LibraryProfile`, while the reusable CLI plan workflow is implemented in
`main.rs` instead of behind a library facade. A native client, alternate CLI,
or third-party integration cannot reuse the useful analyze/plan/command logic
without inheriting application policy and a large dependency graph.

**Idea:**

Create a deliberately small `libalchemist` engine API around versioned input
and output models: `MediaSource`, `AnalysisReport`, `PlanningPolicy`,
`PlanReport`, `CommandSpec`, and stable reason/error codes. Keep analysis,
planning, and command construction independently callable; put FFprobe,
FFmpeg, filesystem, database, and event publication behind adapter traits, and
keep Axum, authentication, SQLite migrations, scheduling, notifications,
updates, and destructive file finalization in the application layer. Start as
a curated facade in the existing package, migrate the CLI `plan` command and
application to consume it, then move the proven boundary into a workspace
package rather than beginning with a high-churn file move.

**First step:**

Add a side-effect-free `PlanEngine::plan(PlanRequest) -> PlanReport` facade that
uses an owned library-neutral profile type, accepts an explicit encoder
inventory, and returns stable decision codes plus an FFmpeg `CommandSpec`.
Port `alchemist plan --json` to that facade and add golden JSON plus
`cargo-semver-checks` coverage before splitting the package.

**Risks / tradeoffs:**

A public library creates a long-term compatibility promise; keep the first
release explicitly pre-1.0, feature-gate process adapters, and do not expose
database rows, Tokio channels, Axum types, or raw FFmpeg implementation details.
