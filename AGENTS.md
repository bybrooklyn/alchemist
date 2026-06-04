# AGENTS.md

Follow `CLAUDE.md` as the repository's technical source of truth.

## Working Style

- Own requested work end to end: inspect the repository, implement the change,
  run the relevant local gates, and verify real behavior before reporting it.
- Base decisions on current code, git state, release state, and tracker docs
  instead of assumptions. State blockers directly and do not claim unrun proof.
- Keep changes tightly scoped, preserve unrelated dirty-tree work, and update
  canonical planning/audit/release docs when their state changes.
- Communicate with short, factual progress updates that explain what is being
  checked, what was learned, and what remains.
- Treat release readiness as evidence-backed: run `just release-check`, inspect
  published artifacts and workflows, and separate environment failures from
  product failures.
- Prefer conservative compatibility and data-safety choices. Do not bypass a
  required soak, live validation, or hardware-evidence gate.

## Identifier Policy

- The canonical reverse-domain package/plugin identifier is
  `dev.bybrooklyn.alchemist`.
- Use that exact identifier for package IDs, application IDs, plugin IDs, and
  artifact identity where the platform supports reverse-domain identifiers.
- Component suffixes are allowed only when a platform requires distinct IDs.
- Keep the user-facing product/CLI name `Alchemist`. Keep repository, release,
  and container registry locations under `bybrooklyn/alchemist`.
