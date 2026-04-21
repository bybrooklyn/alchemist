---
title: Alchemist vs FileFlows
description: Comparing Alchemist and FileFlows for self-hosted video transcoding. Licensing, deployment, config model, and when each is the right choice.
keywords:
  - fileflows alternative
  - fileflows vs alchemist
  - open source fileflows alternative
  - foss transcoding
---

This is an honest, narrow comparison for people evaluating
FileFlows against Alchemist. It stays on the points that
typically drive the decision: licensing, deployment shape,
config model, and what each tool treats as a first-class
feature.

For current FileFlows features and licensing terms, refer to
[fileflows.com](https://fileflows.com/) — this page only
compares against what FileFlows documents publicly.

## At a glance

| | Alchemist | FileFlows |
|---|---|---|
| License | GPLv3 (fully open source) | See FileFlows' own licensing page |
| Config model | Declarative — TOML file and UI settings | Flow editor (node-based) |
| Deployment | Single binary (also a single Docker container) | Server + optional additional processing nodes |
| AV1 target | First-class in the planner | Supported (see FileFlows docs) |
| Hardware acceleration | NVENC, Intel Quick Sync, VAAPI, AMD AMF, Apple VideoToolbox, CPU fallback | Documented hardware support in FileFlows |
| Platforms | Linux, macOS, Windows, Docker | Linux, macOS, Windows, Docker |
| Non-destructive by default | Yes — originals not deleted unless `delete_source` is set | Configurable per flow |

## Choose FileFlows if

- You want to model complex conditional pipelines in a
  graphical flow editor.
- You rely on specific FileFlows plugins or flows you don't
  want to re-express.
- You already have a working FileFlows deployment and no
  concrete reason to change.

## Choose Alchemist if

- **Licensing matters to you.** Alchemist is GPLv3 source,
  binary, and distribution. No paid tier, no license key.
  See [Open Source](/open-source).
- **You prefer declarative config over a flow editor.** The
  planner decides per file whether to skip, remux, or
  transcode based on thresholds and rules you set. See
  [Planner](/planner) and
  [Skip Decisions](/skip-decisions).
- **You want the deployment to be one service.** Alchemist
  is a single binary that embeds its own web UI; no separate
  frontend to run.
- **You care about transparent skip decisions.** Every
  skipped file records the exact reason in plain English
  (BPP, size, codec match, predicted savings).
- **You're transcoding primarily for Jellyfin or Plex.** See
  [Alchemist for Jellyfin](/jellyfin).

## Practical differences

### Config model

FileFlows' flow editor is its core strength and its core
tradeoff. Visual flows make complex branching visible; they
also mean the source of truth for "what does this system
do?" lives in an interactive canvas rather than a text file.

Alchemist is the opposite shape. The source of truth is a
TOML file plus a handful of per-library profiles. Behavior
is determined by a small set of thresholds and rules; the
UI reflects config rather than being where config lives. For
many libraries that's all that's needed. For libraries that
genuinely need conditional graphs, a flow editor is a better
fit.

### Licensing

This is the most common reason people come looking for a
FileFlows alternative. Alchemist is GPLv3 with no paid tier.
Everything in Alchemist is in the public source tree and
stays that way. See [Open Source](/open-source) for the
specifics.

### Hardware selection

Alchemist probes each available encoder at startup and
selects one active device using a deterministic scoring
policy. The probe log shows exactly why every probed backend
succeeded or failed. See [Hardware Acceleration](/hardware).

## Moving off FileFlows

There's no direct import — the abstractions differ. Typical
migration:

1. Install Alchemist next to FileFlows. [Docker](/docker)
   is straightforward.
2. Point it at the same library roots.
3. Run `alchemist plan /path` to see decisions without
   enqueueing. See [Installation](/installation).
4. Re-express conditional flows as
   [Profiles](/profiles) +
   [Stream Rules](/stream-rules) + target codec.
5. Disable FileFlows on the same library once behavior
   matches what you want.

## FAQ

**Is Alchemist free?**
Yes. GPLv3, no paid tier, no license key. Every feature
lives in the public source tree. See
[Open Source](/open-source).

**Does Alchemist have a flow editor?**
No. Configuration is a TOML file plus per-library
[profiles](/profiles) and [stream rules](/stream-rules).
This is intentional — if you specifically need a visual
flow graph, FileFlows remains the better fit.

**Can Alchemist read FileFlows flows?**
No. Migration is a re-configuration, not an import. See the
migration section above for the typical path.

**Does Alchemist support distributed processing?**
Not today. Alchemist runs as a single process that scales
with the host it's on. FileFlows' optional processing nodes
are something Alchemist does not currently try to match.

**How does Alchemist decide what to transcode?**
The [planner](/planner) evaluates each file against
thresholds — bits-per-pixel, minimum file size, target codec
match, predicted savings — and records a plain-English reason
for every skip. See [Skip Decisions](/skip-decisions). This
is the single biggest shift from a flow-based tool.

**Can I try it without touching my library?**
Yes. `alchemist plan /path` runs the full analysis and
reports the per-file decision as text (or `--json`) without
enqueueing any work. See [Installation](/installation).

## See also

- [Alchemist vs Tdarr](/alternatives/tdarr)
- [Open Source](/open-source)
- [Installation](/installation)
- [Alchemist for Jellyfin](/jellyfin)
