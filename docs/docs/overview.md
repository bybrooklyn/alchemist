---
title: Alchemist
description: Alchemist is a self-hosted, GPLv3 transcoding pipeline that scans your media library and encodes only what's worth encoding, using NVENC, Intel Quick Sync, VAAPI, AMF, or Apple VideoToolbox.
slug: /
keywords:
  - self-hosted video transcoding
  - ffmpeg automation
  - jellyfin transcoding
  - av1 transcoding
  - open source transcoding
---

Alchemist scans your media library, analyzes every file, and
decides whether transcoding it would actually save meaningful
space. If a file is already efficiently compressed, it skips
it and tells you exactly why in plain English. If it can save
space without hurting quality, it encodes it using whatever
hardware you have, on your schedule.

Your originals are never touched until the new file passes
quality validation. Nothing is deleted until you say so.

Alchemist is GPLv3 open source. There is no paid tier, no
private "pro" build, no license key, and no phone-home check.

## What it does

- Scans configured library directories and queues files for analysis
- Runs FFprobe on each file to extract codec, resolution, bitrate, and HDR metadata
- Applies BPP (bits-per-pixel) analysis and size thresholds to decide whether transcoding is worth it
- Selects the best available encoder automatically (NVIDIA NVENC, Intel QSV, AMD VAAPI/AMF, Apple VideoToolbox, CPU fallback) and caches valid hardware detection across repeat boots
- Encodes to AV1, HEVC, or H.264 based on your configured target
- Validates output quality (optional VMAF scoring) before promoting the result
- Tells you exactly why every skipped file was skipped
- Supports named API tokens for observability, full automation, and Sonarr/Radarr webhook ingress
- Can be served under a path prefix such as `/alchemist`
- Includes an experimental single-file Conversion / Remux utility with command preview and source/output estimates
- Expands Library Intelligence beyond duplicate detection into storage-focused recommendations
- Sends notifications through Discord, Gotify, ntfy, Telegram, email, or webhooks, with quiet hours for non-critical events

## What it is not

Alchemist is not Tdarr and not FileFlows. It does not try to
be a visual workflow product, a plugin marketplace, or a
general-purpose file automation suite. It is one application
for one job: decide what media is worth optimizing, encode it
safely, and explain every decision.

## Hardware support

Alchemist detects and selects the best available hardware encoder automatically (NVIDIA NVENC, Intel QSV, AMD VAAPI/AMF, Apple VideoToolbox, or CPU fallback). Repeat boots reuse a valid cached detection result when the OS, architecture, FFmpeg/FFprobe versions, and hardware settings have not changed.

For detailed codec support matrices (AV1, HEVC, H.264) and vendor-specific setup guides, see the [Hardware Acceleration](/hardware) documentation.

## Where to start

| Goal | Start here |
|------|-----------|
| Get it running | [Installation](/installation) |
| Docker setup | [Docker](/docker) |
| Get your GPU working | [Hardware](/hardware) |
| Using Jellyfin | [Alchemist for Jellyfin](/jellyfin) |
| Comparing to Tdarr or FileFlows | [Alchemist vs Tdarr](/alternatives/tdarr) · [Alchemist vs FileFlows](/alternatives/fileflows) |
| Why it's GPLv3 | [Open Source](/open-source) |
| Automate with tokens | [API](/api) |
| Understand skip decisions | [Skip Decisions](/skip-decisions) |
| Tune per-library behavior | [Profiles](/profiles) |

## Nightly builds

```bash
docker pull ghcr.io/bybrooklyn/alchemist:nightly
```

Published on every push to `main` that passes Rust checks.
Version format: `0.3.0-dev.3-nightly+abc1234`.
Stable: `ghcr.io/bybrooklyn/alchemist:latest`

## License

GPLv3. Free to use, modify, and distribute under the same
license. Genuinely open source — not source-available.
