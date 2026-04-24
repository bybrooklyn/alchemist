---
name: hygiene
description: Cleans up the Alchemist repo. Removes stale or redundant markdown (drafts, old plans, abandoned TODOs), prunes dead code, stale comments, orphaned files, and unused dependencies. Conservative by default — proposes a plan, waits for approval, then executes. Run /hygiene for a full sweep or /hygiene <area> to focus (e.g. /hygiene md, /hygiene deadcode, /hygiene deps, /hygiene comments).
---

# Repo Hygiene

Sweep the Alchemist repo for cruft — stale markdown, dead code, orphaned files, unused dependencies, obsolete comments — and clean it up. Be conservative: propose, confirm, then delete.

This skill is different from `/audit` (which finds correctness bugs) and `/ideas` (which proposes new work). Hygiene is about subtraction: less code, fewer files, clearer repo.

## Core principle: never delete silently

Anything removed must be in a plan the user saw and approved. No surprise deletions. If in doubt, ask.

## Phase 1: Load context

1. `CLAUDE.md` — understand what files and patterns are load-bearing.
2. Git state: `git status`, `git log --oneline -20` — know what's in flight, don't touch work-in-progress.
3. Quick scan of repo root: what markdown, config, and top-level files exist.

Files that are **always protected** (never propose for deletion without explicit user instruction):

- `README.md`, `LICENSE`, `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `RELEASING.md`
- `CLAUDE.md`, `DESIGN_PHILOSOPHY.md`
- `audit.md`, `ideas.md`, `backlog.md` (these are the project's living state files)
- Anything inside `.github/`, `migrations/`, `tests/`
- `justfile`, `Cargo.toml`, `Cargo.lock`, `package.json`, `package-lock.json`, `pnpm-lock.yaml`, `astro.config.mjs`, `tsconfig.json`
- Any file staged or modified in the current git working tree

## Phase 2: Scope

If the user passed an argument, focus only there:

| Argument | Focus |
|----------|-------|
| `md` | Markdown files only — stale plans, old TODOs, abandoned drafts, duplicate docs |
| `deadcode` | Unused Rust functions/types, dead React components, unreachable branches |
| `deps` | Unused `Cargo.toml` and `package.json` dependencies |
| `comments` | Stale `TODO` / `FIXME` / `XXX`, commented-out code, outdated doc comments |
| `assets` | Orphaned images, unused static files in `web/public/` and `web/src/assets/` |
| `tests` | Tests for code that no longer exists, `#[ignore]`d tests with no linked issue |
| `config` | Unused config fields, obsolete env vars, dead `settings.local.json` entries |
| `gitignored` | Tracked files that should be in `.gitignore`, or vice versa |

Otherwise do all of them.

## Phase 3: What to look for

### Stale markdown (`md`)

Look in the repo root, `docs/`, and any ad-hoc directories for:

- **Planning docs with no remaining TODOs** — `plans.md`, `TODO.md`, `backlog.md`, `seo.md`, `GEMINI.md` etc. If everything in the file is done or superseded, flag for deletion.
- **Duplicate content** — same info in README and another file; prefer README.
- **Draft or scratch files** — names like `notes.md`, `draft.md`, `scratch.md`, `tmp.md`, `WIP.md`, `_old.md`.
- **Docs referencing removed features** — mentions of code, flags, or modules that no longer exist. Grep the referenced symbol; if zero hits in source, doc is stale.
- **One-off AI scratch files** — files clearly written by an assistant for a single task and never referenced since.

For each candidate, report: path, size, last-modified date, last-commit date, a one-line "why stale" reason. Don't delete yet.

### Dead code (`deadcode`)

- **Rust**: run `cargo check --all-targets` and collect `dead_code` warnings. Also grep for `#[allow(dead_code)]` — these are explicit admissions; question whether they're still justified.
- **Unused `pub` items**: items that are `pub` but only used within their own crate. Check if they should be `pub(crate)` or removed.
- **React components**: files in `web/src/components/` with no importers. Use grep for the component name across `web/src/`.
- **Unreachable branches**: `if false`, always-true conditions, match arms after a catch-all.
- **Feature flags with a single variant**: flags where the "off" path has been deleted.

Do NOT auto-delete. Some code is exercised only by tests, examples, or future migrations.

### Unused dependencies (`deps`)

- **Rust**: `cargo machete` if available, else grep each `[dependencies]` name across `src/` and `tests/`. A dep used only in one file with a trivial import is a candidate for evaluation.
- **Node**: `npx depcheck` or grep each `dependencies`/`devDependencies` name across `web/src/` and config files.
- Distinguish between truly unused and transitively required (e.g., types packages, peer deps). When in doubt, flag, don't delete.

### Stale comments (`comments`)

- **`TODO` / `FIXME` / `XXX` / `HACK`** with no date, no ticket, and no obvious follow-up. Grep with `TODO\|FIXME\|XXX\|HACK`.
- **Commented-out code** — lines starting with `//` that look like former code. These belong in git history, not the source.
- **Doc comments referencing removed functions or fields**. Grep the referenced symbol.
- **Copy-paste comments** identical across many files suggesting a stale template.

### Orphaned assets (`assets`)

- Images or static files in `web/public/` or `web/src/assets/` with no references in `web/src/`.
- Old logo variants, unused favicons, screenshots kept "just in case".

### Stale tests (`tests`)

- Tests that reference types/functions no longer in the code (should show as compile errors — a dead giveaway).
- `#[ignore]`d tests with no linked issue in a nearby comment. Either fix, delete, or add a reason.
- Duplicate test names across modules.

### Config rot (`config`)

- Fields in `Config` structs (`src/config.rs`) that are never read. Grep the field name in `src/`.
- `.claude/settings.local.json` entries for tools/paths no longer used.
- Dead env vars documented in CLAUDE.md but never read by the code.

### Gitignore drift (`gitignored`)

- Tracked files matching patterns like `*.log`, `.DS_Store`, `node_modules/`, `target/`, `dist/`, `.env`. Run `git check-ignore -v <path>` to confirm.
- Common cases: build artifacts, editor files, OS metadata.

## Phase 4: Plan before acting

Write a plan to the chat (not a file). Structure:

```
## Hygiene plan

### Proposed deletions
- `path/to/file.md` — reason (12 KB, last edit 4 months ago, all items marked done)
- `src/foo.rs` — `fn old_helper` is dead (0 callers, allow(dead_code) since 2024-11)

### Proposed edits
- `src/bar.rs:123` — remove commented-out block (lines 123–141)
- `Cargo.toml` — drop `unused_crate` from dependencies

### Skipped (flagged but uncertain)
- `docs/migration-v0.2.md` — references v0.2, unclear if still relevant to upgrade guide

### Nothing to do in
- deps, tests (already clean)
```

Under each section, keep entries to one line + reason. No walls of text.

**Wait for the user to approve, narrow, or reject before executing.** If they say "go ahead" execute everything in "Proposed deletions" and "Proposed edits". If they say "only the md ones", execute only that subset.

## Phase 5: Execute

Once approved:

1. For each deletion: `git rm <path>` (not `rm` — keeps git aware).
2. For each edit: use `Edit` tool with precise before/after.
3. For dependency removal: edit the manifest, then run `cargo check` or the frontend equivalent to confirm build still works.
4. Do NOT commit automatically. Leave changes staged/modified so the user can review `git diff` before committing.
5. If any step fails (e.g. removing a dep breaks the build), stop, revert that one change, and report.

## Phase 6: Verify

After executing, run in parallel where possible:

- `just check-rust` (fmt + clippy + build) if Rust files changed
- `just check-web` if frontend files changed
- `git status` to show what's pending

If any check fails, report the failure and the exact file/line; do not try to auto-fix beyond reverting the specific hygiene change that caused it.

## Phase 7: Summary report

One short message:

1. Files deleted (count + list).
2. Edits made (count + summary, e.g. "3 dead imports, 1 commented block, 2 TODOs").
3. Items deferred (with reason per category).
4. Build/check status.
5. Suggested commit message (but do NOT commit — user commits manually).

Under 200 words. Details are visible in `git diff`.

## Constraints

- Schema migrations in `migrations/` are **never** deleted, even if superseded. History is load-bearing.
- Don't delete anything modified in the current working tree — it's likely WIP.
- Don't delete anything referenced by `CLAUDE.md` unless you also update `CLAUDE.md`.
- Don't touch `.git/`, `target/`, `node_modules/`, or other generated directories.
- Cross-platform rule still applies: don't remove a file just because it looks Windows-specific if you're on macOS (or vice versa).
- If a file's purpose is genuinely ambiguous, ask the user rather than guessing.

## Anti-patterns

- Deleting markdown that's "old" without checking whether it's linked from README or docs.
- Removing `#[allow(dead_code)]` items without checking for test-only or example use.
- Auto-committing cleanup changes — user reviews before commit.
- Mass-renaming or "while I'm in here" edits outside hygiene scope.
- Rewriting prose in kept docs — hygiene is subtraction, not editing.
- Using `rm` instead of `git rm` (loses git's awareness).
