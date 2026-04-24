---
name: ideas
description: Brainstorms new feature, UX, integration, and polish ideas for Alchemist. Writes findings to ideas.md with category, effort estimate, and rationale. Run /ideas for a full sweep or /ideas <category> to focus (e.g. /ideas features, /ideas ux, /ideas integrations, /ideas performance, /ideas polish).
---

# Alchemist Idea Generation

Generate genuinely new, useful ideas for Alchemist. Think like a user and a product owner — not a bug-finder. Write every idea to `ideas.md` with category, effort estimate, rationale, and a concrete first-step sketch.

This skill is the counterpart to `/audit`: audit finds what's broken, ideas finds what's missing or could be better.

## Phase 1: Read Context

**Always start here.** Before generating anything, read in this order:

1. `CLAUDE.md` — architecture, stack, binding constraints (additive schema, no data loss, cross-platform).
2. `README.md` — the outward-facing feature list. Anything already listed is NOT a new idea.
3. `ideas.md` — if it exists, extract every existing idea ID (F-1, UX-2, INT-3, etc.) and their titles. You must not write duplicates. If it doesn't exist, you'll create it.
4. `audit.md` — known bugs and gaps. Do NOT turn audit items into ideas; they already have a home. But note the themes (e.g. "lots of SSE issues") so you can propose structural improvements that retire whole classes of audit findings.
5. `DESIGN_PHILOSOPHY.md` if present — ideas must respect these principles.

Skim `src/` top-level structure and `web/src/pages/` to know what surfaces already exist.

## Phase 2: Scope

If the user passed an argument (e.g. `/ideas ux`), only generate ideas in that category. Otherwise cover all categories.

Category arguments map to:

| Argument | Focus |
|----------|-------|
| `features` | New user-facing capabilities (new transcode modes, new job workflows, new filters) |
| `ux` | Improvements to existing UI, discoverability, feedback, onboarding |
| `integrations` | Arrs (Sonarr/Radarr), Plex/Jellyfin/Emby, Tdarr parity, notification channels, webhooks |
| `performance` | Throughput, latency, resource efficiency (not bugfixes — design-level wins) |
| `polish` | Small quality-of-life touches: tooltips, keyboard shortcuts, empty states, error copy |
| `observability` | Metrics, tracing, logs, health checks, Prometheus/OTEL, dashboards |
| `operator` | Self-hosting ergonomics: backups, restore, config export/import, multi-instance |
| `encoding` | Codec/preset/filter capabilities: new encoders, HDR handling, audio, subtitles |
| `automation` | Rules engine, conditional workflows, scheduled actions, policies |

## Phase 3: Idea Quality Bar

An idea is worth writing down only if it passes ALL of these:

1. **Not already in the product.** Check `README.md`, settings UI list in `web/src/components/SettingsPanel.tsx` if present, and actual code — don't propose something that already exists.
2. **Not already an audit item.** If it's a bug fix or gap closure, it belongs in `audit.md`, not here.
3. **Not already in `ideas.md`.** Search the file first.
4. **Concrete.** "Make it better" is not an idea. "Add a per-folder override for target codec in the watch folder settings panel" is.
5. **Fits the stack and constraints.** No "rewrite in Go", no "add a Kubernetes operator" unless there's a real reason. Respect cross-platform, additive-schema, no-data-loss rules.
6. **A real user would want it.** Frame the rationale around a self-hosted media nerd running this on their homelab — not an enterprise fantasy.

Reject ideas that are:
- Vague ("improve error handling")
- Speculative research ("investigate ML-based encoding")
- Scope explosions ("add a plugin system") unless there's a clearly enumerated use case
- Already covered by FFmpeg flags the user can set — unless surfacing them is the idea

## Phase 4: Idea Categories — What To Look For

### Features
- Workflows that currently require manual steps (batch re-analyze, preset templates)
- Media-type-specific modes (anime, live-action, concerts, screen recordings)
- Reversibility tools (undo-last-encode, dry-run mode with diff)
- Library-level operations (dedupe, orphan detection, rename-in-place)

### UX
- Pages that need filters, sort, or saved views
- Modals that dump data without structure (job detail, hardware info)
- First-run and empty states
- Destructive-action confirmations
- Keyboard-driven navigation for power users
- Copy polish — error messages that tell the user what to do

### Integrations
- Sonarr/Radarr webhook triggers (encode on import)
- Plex/Jellyfin library refresh hooks
- Tdarr-style feature parity gaps worth closing
- Home Assistant, Prometheus/Grafana
- Additional notification targets (ntfy, Matrix, Pushover)
- Import/export config as YAML/JSON for GitOps users

### Performance
- Parallelism within a single encode (per-stream, per-chapter)
- Pre-fetching analysis during idle time
- Smart job ordering (group by source drive to reduce seeks)
- Caching FFprobe output across scans if file mtime+size unchanged
- Streaming progress without polling

### Polish
- Tooltips on every abbreviation
- Sticky toolbar/header when scrolling long tables
- Bulk selection with shift-click
- Copy-to-clipboard on hashes, paths, job IDs
- Relative and absolute time on hover
- Dark/light theme refinement
- Mobile viewport behavior

### Observability
- Per-encoder success/failure rate over time
- Savings by folder / codec / resolution
- Queue depth and throughput graphs
- Export metrics in Prometheus format
- Structured JSON logging mode

### Operator
- One-click database backup and restore
- Config dry-run (validate without applying)
- Settings diff viewer between versions
- Stateful healthcheck endpoint for Docker/k8s
- Upgrade notes surfaced in UI when a new version is available

### Encoding
- Tune options per content type (film, animation, grain)
- Two-pass mode for target-bitrate encodes
- HDR → SDR tonemap presets
- Audio normalization / downmix options surfaced in UI
- Subtitle extraction and re-muxing options
- Chapter preservation verification

### Automation
- Rules: "if folder = Anime and bitrate > X, use preset Y"
- Scheduled re-scans
- Auto-pause when on battery (laptop users)
- Auto-resume when system idle for N minutes
- Conditional notifications (only notify on P1 failures)

## Phase 5: Severity / Size Classification

Every idea gets a **category code** and a **size tier**:

**Category codes** (pick one primary — use for the ID prefix):

| Prefix | Category |
|--------|----------|
| F | Feature |
| UX | UX / Interface |
| INT | Integration |
| PERF | Performance |
| POL | Polish |
| OBS | Observability |
| OP | Operator |
| ENC | Encoding |
| AUTO | Automation |

**Size tiers:**

| Tier | Meaning |
|------|---------|
| S | Small — under a day of focused work |
| M | Medium — 1–3 days, touches a few modules |
| L | Large — a week or more, crosses backend + frontend + schema |
| XL | Major — multi-week, architectural |

## Phase 6: Write to ideas.md

### Rules

1. **Read ideas.md first.** Find the highest existing ID per prefix (e.g. if F-3 exists, next is F-4).
2. **No duplicates.** If the same idea already has an entry, skip it.
3. **Append to the correct section.** Each category has its own `## Features`, `## UX`, etc. section.
4. **If the file doesn't exist**, create it with a header, a "Last updated" line, and all category sections (even empty ones — makes future adds easier).
5. **Update "Last updated"** at the top.
6. **Top-of-file "Top picks" list** — keep the 5 highest-impact-to-effort ideas pinned at the top. Replace the weakest pick if a new idea beats it. If no "Top picks" exists, create it.

### File skeleton (create if missing)

```markdown
# Alchemist — Ideas

*Forward-looking ideas for features, UX, integrations, and polish. Bugs go in `audit.md`.*

**Last updated:** YYYY-MM-DD

## Top picks

1. [ID] Short title — one-sentence why
2. ...

## Features
## UX
## Integrations
## Performance
## Polish
## Observability
## Operator
## Encoding
## Automation
```

### Entry format

Every entry follows this exact structure:

```markdown
### [PREFIX-N] Short title

**Category:** Features / UX / Integration / Performance / Polish / Observability / Operator / Encoding / Automation
**Size:** S / M / L / XL
**Touches:** backend / frontend / schema / config / docs (list what changes)

**Problem or gap:**

2–4 sentences describing what's missing or suboptimal today. Reference a real user scenario.
Name the file or page where the gap lives if applicable.

**Idea:**

2–5 sentences describing the proposed solution. Be specific about UI placement,
API shape, config keys, or schema additions.

**First step:**

One concrete, small action that would validate or begin the work — a prototype, a
config flag, a spike. Not the whole thing.

**Risks / tradeoffs:**

One line. What could go wrong, who this might annoy, or what it conflicts with.
Skip if genuinely none.
```

## Phase 7: Summary Report

After writing to `ideas.md`, report:

1. How many new ideas were added, broken down by category.
2. The IDs and one-line titles of each new entry.
3. Which existing ideas (if any) are now implemented — mark them `**Status: SHIPPED**` in `ideas.md`.
4. What changed in "Top picks" and why.
5. The single idea you'd start with and a one-sentence why.

Keep the report under 250 words. The user can read `ideas.md` for full details.

## Constraints (from CLAUDE.md)

Ideas must respect these or be explicitly flagged as "requires relaxing X":

- Cross-platform (macOS / Linux / Windows) — no Linux-only ideas without a note
- Schema changes additive only — no renames or drops
- Never overwrite user media by default
- No data loss on failure
- Databases from v0.2.5+ must remain usable
- No `.unwrap()` / `.expect()` in production paths

## Anti-patterns to avoid

- Turning audit items into ideas (they already have a home)
- Proposing "a plugin system" as a shortcut to avoid designing the actual feature
- Recommending new dependencies without naming them
- Suggesting paid-tier or SaaS features for a self-hosted project
- Ideas that are really just "follow TODO comments in code"
- Long strategic essays — each idea is 10–20 lines, not an RFC
