# Alchemist

Point it at your media library. Walk away. Come back to a smaller, better-encoded collection.

## Start Here

- [Get Started](getting-started/installation.md)
- [View on GitHub](https://github.com/bybrooklyn/alchemist)

## What Alchemist does

Alchemist scans your media library, analyzes every file, and
decides whether transcoding it would actually save meaningful
space. If the file is already efficiently compressed, it skips
it and tells you exactly why in plain English. If it can save
space without hurting quality, it encodes it - automatically,
using whatever hardware you have.

Your originals are never touched until the new file passes
quality validation. Nothing is deleted until you say so.

### Hardware acceleration

NVIDIA NVENC, Intel QSV, AMD VAAPI/AMF, and Apple
VideoToolbox detected and used automatically. CPU fallback
when no GPU is available - no manual setup required.

### Intelligent skipping

Bits-per-pixel analysis, size reduction thresholds, and
codec-awareness mean Alchemist only encodes files that will
actually get meaningfully smaller. Every skip is explained.

### Per-library profiles

Different rules for movies, TV shows, and home videos.
Four built-in presets - Space Saver, Balanced, Quality
First, Streaming - fully customizable.

### Single binary

One file. No services to install, no plugin stacks, no
databases to manage separately. Docker image bundles
everything including FFmpeg.

## Who this is for

Alchemist is built for self-hosters who run Plex, Jellyfin,
or Emby and want to reclaim storage without babysitting shell
commands. If you have a media library measured in terabytes
and you want it to get smaller on its own while you sleep, this
is the tool.

It is not a Tdarr replacement with flow editors and plugins.
It is not a commercial service. It is a GPLv3 open source tool
that does one thing - and does it without asking you to become
an FFmpeg expert.

## Where to start

- [Docker install](getting-started/installation.md): The fastest path to a running instance.
- [Hardware setup](guides/hardware.md): Get NVIDIA, Intel, AMD, or Apple acceleration working.
- [Library profiles](guides/profiles.md): Different rules for different folders.
- [Why did it skip my file?](reference/skip-decisions.md): Understand every skip decision Alchemist makes.

## Hardware at a glance

| Vendor | Encoders | Notes |
|--------|----------|-------|
| NVIDIA | AV1, HEVC, H.264 (NVENC) | RTX 30/40 for AV1 |
| Intel | AV1, HEVC, H.264 (QSV) | 12th gen+ for AV1 |
| AMD | HEVC, H.264 (VAAPI/AMF) | RDNA 2+ for AV1 |
| Apple | HEVC, H.264 (VideoToolbox) | M3+ for AV1 |
| CPU | AV1 (SVT-AV1), HEVC (x265), H.264 (x264) | Always available |

## Nightly builds

Nightly builds are published automatically on every push to
`main` after Rust checks pass.

```bash
docker pull ghcr.io/bybrooklyn/alchemist:nightly
```

Stable releases are tagged `vX.Y.Z`. See the
[Changelog](reference/changelog.md) for what changed.
