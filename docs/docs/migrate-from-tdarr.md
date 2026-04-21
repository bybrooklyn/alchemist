---
title: Migrate from Tdarr
description: Practical steps for moving a working Tdarr setup to Alchemist. Install side-by-side, dry-run against the same library, re-express flows as profiles and stream rules, then cut over.
keywords:
  - migrate from tdarr
  - tdarr to alchemist
  - replace tdarr
  - tdarr migration
slug: /migrate-from-tdarr
---

This is a step-by-step guide for moving a working Tdarr
deployment to Alchemist. It assumes you're comfortable
editing a config file and have an existing Tdarr setup you
want to replace, not supplement.

There is no direct import. The two tools use different
abstractions — Tdarr has a flow/plugin model, Alchemist uses
declarative profiles and stream rules. Migration is a
re-configuration exercise, not a file conversion. For
per-feature comparison see
[Alchemist vs Tdarr](/alternatives/tdarr).

## Before you start

A short checklist keeps the cutover boring:

- A recent backup, or a clear understanding of which files
  Tdarr might have modified.
- Know your target codec (AV1 / HEVC / H.264). See
  [Codecs](/codecs) if you haven't decided.
- Know which hardware encoder Tdarr currently uses (NVENC,
  Quick Sync, VAAPI, AMF, VideoToolbox) — Alchemist will
  auto-detect, but it's useful to know what's expected.
- Enough free space on the output volume to hold in-progress
  encodes while originals are still on disk (Alchemist is
  non-destructive by default).

## Step 1 — Install Alchemist alongside Tdarr

Alchemist does not require Tdarr to stop. Install it in
parallel on the same host (or a separate one) and give it the
same library roots. [Docker](/docker) is the common path:

```yaml
services:
  alchemist:
    image: ghcr.io/bybrooklyn/alchemist:latest
    container_name: alchemist
    ports:
      - "3000:3000"
    volumes:
      - ~/.config/alchemist:/app/config
      - ~/.config/alchemist:/app/data
      - /path/to/media:/media
    restart: unless-stopped
```

For GPU passthrough see
[GPU Passthrough](/gpu-passthrough). Complete the
[First Run](/first-run) wizard to create the first account
and point it at your library directories.

## Step 2 — Dry-run against the same library

Before enqueueing anything, use the `plan` subcommand. It
scans and analyses every file and reports the decision
(skip / remux / transcode) as text or JSON — without writing
jobs to the queue.

```bash
alchemist plan /path/to/media
alchemist plan /path/to/media --json
```

This is the single most useful step in the migration. It
answers "what would Alchemist do?" per file without making
any changes. Skim the output for surprises:

- Files you expected to transcode that are being skipped.
- Files you wanted skipped that are being queued.
- Codec / container combinations that end up remuxed rather
  than re-encoded (see
  [already_target_codec_wrong_container](/planner#already_target_codec_wrong_container)).

Unexpected skips are almost always one of the reasons in
[Skip Decisions](/skip-decisions) — BPP below threshold,
codec already matches, or the file is below the minimum file
size.

## Step 3 — Re-express flows as Alchemist config

Tdarr plugins and flows don't port directly. They map to
three concepts in Alchemist:

| Tdarr concept | Alchemist equivalent |
|---|---|
| "Target codec" plugin / flow step | Target codec in [Profile](/profiles) |
| Per-stream language / title logic | [Stream rules](/stream-rules) |
| Size, bitrate, or BPP thresholds | Planner thresholds — [Planner](/planner) |
| "Skip if already HEVC/AV1" | Built-in — the planner skips files already in the target codec |
| Container remux | Handled automatically when codec matches but container does not |

Most migrations come down to:

1. Set the right **target codec** for each library (movies,
   TV, or a single shared profile).
2. Express any per-library size / bitrate / BPP cutoffs as
   planner thresholds in
   [Configuration Reference](/configuration-reference).
3. Re-create language, title, or audio-handling rules as
   [stream rules](/stream-rules).
4. Decide whether to keep originals (default) or enable
   `delete_source`.

## Step 4 — Run a small batch first

Before turning Alchemist loose on the whole library, start
with a small target:

- Pick one directory or a small sub-library.
- Let Alchemist queue and process it.
- Spot-check a few outputs for quality and container
  correctness.
- Verify Jellyfin / Plex / whatever consumes the files picks
  them up cleanly.

VMAF scoring (if configured) can gate promotion of the
encoded output automatically — see
[quality settings](/configuration-reference#quality).

## Step 5 — Disable Tdarr on the same scope

Once Alchemist's decisions match what you want:

- Stop Tdarr from processing the same library roots. Leave
  Tdarr running on other scopes if you still need it
  elsewhere.
- If you were using Tdarr for its node architecture and you
  don't need cross-host scaling anymore, shut the nodes
  down.
- Keep originals until you're confident in the new pipeline.
  Originals are what make the migration reversible.

## Step 6 — Optional cleanup

Only after a reasonable observation period:

- Delete originals, or enable `delete_source` for new jobs.
- Uninstall Tdarr / remove its container.
- Remove the Tdarr volumes if you no longer need its
  database.

## Common migration surprises

**"My whole library gets skipped."**
Usually means the library is already in the target codec, or
`min_bpp_threshold` is too aggressive. See
[Skip Decisions](/skip-decisions) — every skip has an
explanation attached.

**"The wrong GPU is being used."**
Alchemist auto-selects. Override in
**Settings → Hardware**. See
[Hardware Acceleration](/hardware).

**"Jobs are stuck in Queued."**
Most often the engine is paused or the schedule window is
closed. See
[Troubleshooting — Jobs stuck in Queued](/troubleshooting#jobs-stuck-in-queued).

**"CPU fallback instead of my GPU."**
See
[CPU fallback despite GPU](/troubleshooting#cpu-fallback-despite-gpu)
and the vendor-specific guide under
[Hardware](/hardware).

## See also

- [Alchemist vs Tdarr](/alternatives/tdarr) — feature-by-feature comparison.
- [Installation](/installation) — binary / Docker / source.
- [First Run](/first-run) — the setup wizard.
- [Profiles](/profiles) and [Stream Rules](/stream-rules) —
  where Tdarr flows get re-expressed.
- [Planner](/planner) — the decision logic.
- [Open Source](/open-source) — licensing.
