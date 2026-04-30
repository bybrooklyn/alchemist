---
title: Alternatives
description: Side-by-side comparisons between Alchemist and other self-hosted transcoding tools, focused on open-source licensing, deployment shape, and config model.
keywords:
  - self-hosted transcoder comparison
  - tdarr alternative
  - fileflows alternative
  - open source transcoder
slug: /alternatives/
---

Alchemist is one of several self-hosted tools that automate
media transcoding. This section compares it against the
alternatives people most often look at, without pretending
the differences are cosmetic.

Each comparison stays narrow on purpose: licensing,
deployment shape, configuration model, and where each tool
is a better fit. Feature tables only include claims we can
point at in our own code or that each vendor documents on
their own site. No rumours, no invented benchmarks, no soft
pedaling paid tiers or architectural overhead.

## Comparisons

- [Alchemist vs Tdarr](/alternatives/tdarr) — node-based vs
  single-binary, flows vs declarative config, when each is
  the better pick.
- [Alchemist vs FileFlows](/alternatives/fileflows) — flow
  editor vs TOML + profiles, licensing differences,
  migration notes.

## What Alchemist optimises for

The short version, so the comparison pages don't have to
repeat it:

- **GPLv3, no paid tier.** Every feature lives in the public
  source tree. No license key, no private feature unlock. See
  [Open Source](/open-source).
- **One process.** A single binary with the web UI embedded —
  no separate server and node processes, no license server.
  See [Installation](/installation).
- **Declarative.** A TOML file plus per-library
  [profiles](/profiles) and
  [stream rules](/stream-rules) — no visual flow editor.
- **Non-destructive by default.** Originals are not deleted
  unless you explicitly opt in. See
  [Configuration Reference](/configuration-reference).
- **Transparent skip logic.** Every skipped file records the
  exact reason — BPP, codec match, size threshold, predicted
  savings. See [Skip Decisions](/skip-decisions).

## Where the comparisons stop

These pages don't try to score tools on features you can
read about on each project's own site. If you want the
authoritative list of what Tdarr or FileFlows supports, their
own documentation is the source of truth; we link out for
the specifics we don't independently verify. Where a
competitor documents pricing tiers, license-key handling,
server/node architecture, workers, or flow/plugin models, we
call that out directly.

## Related

- [Migrating from Tdarr](/migrate-from-tdarr) — practical
  steps for moving a working Tdarr setup to Alchemist.
- [Alchemist for Jellyfin](/jellyfin) — if your library
  mostly serves Jellyfin.
- [Hardware Acceleration](/hardware) — which GPU paths
  Alchemist supports.
