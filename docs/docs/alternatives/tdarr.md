---
title: Alchemist vs Tdarr
description: How Alchemist compares to Tdarr for self-hosted video transcoding automation. Licensing, deployment model, hardware support, and when each tool is the better fit.
keywords:
  - tdarr alternative
  - tdarr vs alchemist
  - free tdarr alternative
  - open source tdarr alternative
---

This is an honest, narrow comparison for people who already
know what Tdarr is and are evaluating alternatives. It
covers the questions that actually come up when switching:
licensing, deployment shape, hardware coverage, and where
each tool is a better fit.

For current Tdarr features and licensing terms, refer to
[tdarr.io](https://home.tdarr.io/) — this page only compares
against what Tdarr documents publicly.

## At a glance

| | Alchemist | Tdarr |
|---|---|---|
| License | GPLv3 (fully open source) | See Tdarr's own licensing page |
| Deployment | Single binary (also a single Docker container) | Server + node(s) |
| Config model | Declarative — TOML file and UI settings | Plugin stack / flow editor |
| AV1 target | First-class in the planner, uses AV1-capable GPUs when available | Supported via plugins/flows |
| Hardware acceleration | NVENC, Intel Quick Sync, VAAPI, AMD AMF, Apple VideoToolbox, CPU fallback | NVENC, Quick Sync, VAAPI, VideoToolbox (see Tdarr docs) |
| Platforms | Linux, macOS, Windows, Docker | Linux, macOS, Windows, Docker |
| Non-destructive by default | Yes — `delete_source` is off by default, output written alongside or to a mirrored root | Configurable per workflow |
| Scaling model | Scales with a single host's concurrency | Scales horizontally with additional nodes |

## Choose Tdarr if

- You want to distribute transcoding across multiple
  physical machines (the node architecture is Tdarr's core
  strength).
- You rely on specific community Tdarr plugins or flows and
  don't want to re-express them as Alchemist settings.
- You already have a working Tdarr deployment and no concrete
  reason to change.

## Choose Alchemist if

- **You want a single binary to deploy.** Alchemist is one
  service. There's no node to register, no separate UI
  server, no plugin repository to pin.
- **Licensing matters to you.** Alchemist is GPLv3
  end-to-end — source, binary, and everything it does. No
  paid tier, no license key, no phone-home.
- **You prefer declarative config over flow editors.** The
  planner decides per file whether to skip, remux, or
  transcode, using thresholds you set in a TOML file or the
  UI. See [Planner](/planner) and
  [Skip Decisions](/skip-decisions).
- **You care about reversibility.** Originals are not
  deleted unless you turn that on explicitly. VMAF can
  optionally gate the promote step.
- **You're transcoding primarily for Jellyfin or Plex.** See
  [Alchemist for Jellyfin](/jellyfin).

## Practical differences

### Deployment shape

Tdarr has a server process and one or more node processes.
That maps well onto a homelab that already spans multiple
machines, and poorly onto a homelab that doesn't — the
server/node split is overhead you carry for the option of
scaling out later.

Alchemist is one process. Concurrency is bounded by the host
it runs on. If you want to scale across machines you'd run
Alchemist on each machine with its own library roots, or
share the library over a network filesystem.

### Skip decisions

Alchemist skips files that are already efficient and
surfaces the exact reason in the **Skipped** tab — BPP below
threshold, already in target codec, below minimum file size,
or predicted savings below the configured threshold. This is
the single most common source of confusion when moving from
a flow-based tool; most "why didn't this transcode?"
questions resolve against
[Skip Decisions](/skip-decisions).

### Hardware selection

Both tools probe FFmpeg for available encoders. Alchemist
runs a short test encode per backend at startup and selects
one active device using a deterministic scoring policy (see
[Hardware Acceleration](/hardware)). The probe log is
visible in **Settings → Hardware** and records exactly why a
backend failed.

## Moving off Tdarr

There isn't a one-click import — the two tools have
different abstractions. The practical path:

1. Install Alchemist alongside Tdarr
   ([Docker](/docker) works well).
2. Point Alchemist at the same library roots.
3. Run it in dry mode (`alchemist plan /path`) to see the
   skip/remux/transcode decision for every file without
   enqueueing anything. See [Installation](/installation).
4. Tune [Profiles](/profiles) and
   [Stream Rules](/stream-rules) until the decisions match
   what you want.
5. Disable Tdarr on the same library once you're satisfied.

## FAQ

**Is Alchemist a drop-in replacement for Tdarr?**
No. Tdarr's flow/plugin model doesn't have a one-to-one
equivalent in Alchemist. Most real-world Tdarr setups are
expressible as Alchemist profiles + stream rules + a target
codec, but migration is a config exercise, not an import.
See [Migrating from Tdarr](/migrate-from-tdarr).

**Does Alchemist support distributed nodes?**
Not today. Alchemist runs as a single process that scales
with the host it's on. If you need horizontal scaling across
multiple physical machines, that remains a Tdarr strength.

**Is Alchemist free?**
Yes. GPLv3, no paid tier, no license key, no phone-home
check. The binary you install is built from the same code in
the repository. See [Open Source](/open-source).

**Can I run both at the same time during migration?**
Yes. Running Alchemist in dry-run mode
(`alchemist plan /path`) against the same library roots
doesn't touch files — it reports the decision per file. Once
satisfied, point Alchemist at the library and disable Tdarr
on the same scope. See [Installation](/installation) for the
`plan` subcommand.

**Which tool handles AV1 better?**
Both can target AV1. Alchemist treats AV1 as a first-class
output codec in its [planner](/planner) and will pick an
AV1-capable hardware encoder when one is present — av1_nvenc
(RTX 30/40), av1_qsv (Intel 12th gen+), av1_vaapi, av1_amf,
or av1_videotoolbox (Apple M3+). See [AV1](/av1).

**Does Alchemist have a flow editor?**
No. Configuration is declarative — a TOML file plus
per-library [profiles](/profiles) and
[stream rules](/stream-rules). That's the intentional
difference; if you specifically want a flow editor, Tdarr
or FileFlows are a better fit.

## See also

- [Migrating from Tdarr](/migrate-from-tdarr) — step-by-step
  guide for moving a library across.
- [Alchemist vs FileFlows](/alternatives/fileflows)
- [Open Source](/open-source)
- [Installation](/installation)
- [Alchemist for Jellyfin](/jellyfin)
