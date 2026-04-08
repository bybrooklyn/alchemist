---
title: Alchemist Overview
description: What Alchemist is, what it does, and where to start.
slug: /
---

Alchemist scans your media library, analyzes every file, and
decides whether transcoding it would actually save meaningful
space. If a file is already efficiently compressed, it skips
it and tells you exactly why in plain English. If it can save
space without hurting quality, it encodes it — using whatever
hardware you have — automatically, while you sleep.

Your originals are never touched until the new file passes
quality validation. Nothing is deleted until you say so.

## What it does

- Scans configured library directories and queues files for analysis
- Runs FFprobe on each file to extract codec, resolution, bitrate, and HDR metadata
- Applies BPP (bits-per-pixel) analysis and size thresholds to decide whether transcoding is worth it
- Selects the best available encoder automatically (NVIDIA NVENC, Intel QSV, AMD VAAPI/AMF, Apple VideoToolbox, CPU fallback)
- Encodes to AV1, HEVC, or H.264 based on your configured target
- Validates output quality (optional VMAF scoring) before promoting the result
- Tells you exactly why every skipped file was skipped
- Supports named API tokens for automation clients and external observability
- Can be served under a path prefix such as `/alchemist`
- Includes an experimental single-file Conversion / Remux workflow
- Expands Library Intelligence beyond duplicate detection into storage-focused recommendations

## What it is not

Alchemist is not Tdarr. There are no flow editors, no plugin
stacks, no separate services to install. It is a single
binary that does one thing without asking you to become an
FFmpeg expert.

## Hardware support

| Vendor | AV1 | HEVC | H.264 | Notes |
|--------|-----|------|-------|-------|
| NVIDIA NVENC | RTX 30/40 | Maxwell+ | All | Best for speed |
| Intel QSV | 12th gen+ | 6th gen+ | All | Best for power efficiency |
| AMD VAAPI/AMF | RDNA 2+ on compatible driver/FFmpeg stacks | Polaris+ | All | Linux VAAPI / Windows AMF; HEVC/H.264 are the validated AMD paths for `0.3.0` |
| Apple VideoToolbox | M3+ | M1+ / T2 | All | Binary install recommended |
| CPU (SVT-AV1/x265/x264) | All | All | All | Always available |

## Where to start

| Goal | Start here |
|------|-----------|
| Get it running | [Installation](/installation) |
| Docker setup | [Docker](/docker) |
| Get your GPU working | [Hardware](/hardware) |
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
