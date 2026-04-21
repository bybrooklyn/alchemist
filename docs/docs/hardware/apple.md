---
title: Apple
description: Configure Apple VideoToolbox for Alchemist on macOS. Native binary installs recommended — Docker on macOS has limited VideoToolbox access.
keywords:
  - apple videotoolbox
  - apple silicon transcoding
  - macos ffmpeg hardware encoding
---

Apple VideoToolbox is the native macOS hardware encode path.
Binary installs are strongly recommended. Docker on macOS
has limited VideoToolbox access and is not the reliable path
for production encoding.

## Supported hardware

| Hardware | H.264 | HEVC | AV1 | Notes |
|---------|-------|------|-----|------|
| Intel + T2 | Yes | Yes | No | HEVC depends on T2-capable hardware |
| M1 / M2 | Yes | Yes | No | Native media engines |
| M3+ | Yes | Yes | Yes | AV1 encode support |

## Install path

Use the Alchemist macOS binary plus a Homebrew FFmpeg build:

```bash
brew install ffmpeg
ffmpeg -encoders | grep videotoolbox
```

Expected encoders include:

- `h264_videotoolbox`
- `hevc_videotoolbox`
- `av1_videotoolbox` on M3+

## Critical probe note

VideoToolbox fails with error `-12908` if you probe it with
a synthetic `lavfi` frame without `-allow_sw 1` and
`-vf format=yuv420p`. Alchemist `0.3.0` includes that fix in
hardware detection automatically.

If you want to verify the probe manually, use this exact
command:

```bash
ffmpeg -f lavfi -i color=c=black:s=64x64:d=0.1 \
  -vf format=yuv420p \
  -c:v hevc_videotoolbox \
  -allow_sw 1 \
  -frames:v 1 -f null -
```

## In Alchemist

Set **Settings → Hardware → Preferred Vendor → apple**.
Do not set a device path. VideoToolbox is not exposed as a
Linux-style render node.
