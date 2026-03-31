---
title: Profiles
description: Per-library transcoding profiles and what each controls.
---

Profiles define how Alchemist handles files in a given
directory. Each watch directory gets its own profile.

## Built-in presets

| Profile | Codec | Best for |
|---------|-------|----------|
| Space Saver | AV1 | Old TV, content you rarely rewatch |
| Balanced | AV1 | General libraries (default) |
| Quality First | HEVC | Movies, anything you care about |
| Streaming | H.264 | Remote access, older or limited devices |

## What profiles control

- **Output codec** — AV1, HEVC, or H.264
- **Quality profile** — speed/quality tradeoff preset
- **BPP threshold** — minimum bits-per-pixel to transcode
- **Size reduction threshold** — minimum predicted savings
- **Min file size** — skip files below this (MB)
- **Stream rules** — which audio tracks to keep or strip
- **Subtitle mode** — copy, burn, extract, or drop
- **HDR mode** — preserve metadata or tonemap to SDR

## Assigning profiles

Select a profile when adding a directory in
**Settings → Library**. Changeable at any time — jobs
already queued use the profile they were planned with.

## Smart skipping

Files already meeting the profile's targets are skipped
automatically. Every skip is recorded with a reason. See
[Skip Decisions](/skip-decisions).
