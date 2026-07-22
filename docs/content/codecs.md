---
title: Codecs
description: Choosing between AV1, HEVC, and H.264 as a transcoding target. Hardware support per vendor, and how Alchemist uses bits-per-pixel (BPP) instead of raw bitrate.
keywords:
  - av1 vs hevc
  - hevc vs h264
  - bits per pixel
  - transcoding codec choice
---

Alchemist targets three video codecs: AV1, HEVC, and H.264.
They are not interchangeable. Pick based on storage,
playback compatibility, and available hardware.

## Comparison

| Codec | File size | Quality efficiency | Compatibility | Encoding speed |
|------|-----------|--------------------|---------------|----------------|
| AV1 | Smallest | Best | Growing | Slowest |
| HEVC | Smaller | Very high | Good on modern devices | Medium |
| H.264 | Largest | Lowest of the three | Universal | Fastest |

## When to choose AV1

Use AV1 when saving the most space matters and your playback
devices are modern enough to handle it.

Hardware support:

- NVIDIA: RTX 40 (Ada) NVENC — RTX 30 (Ampere) decodes AV1 but cannot encode it
- Intel: Arc, or integrated from Meteor Lake / Core Ultra onward (earlier iGPUs have no AV1 encoder)
- AMD: RDNA 3+ depending on driver/FFmpeg stack; HEVC/H.264 are the validated AMD paths, while AV1 remains driver/FFmpeg-stack sensitive (RDNA 2 is AV1 decode-only)
- Apple: no AV1 hardware encoder on any Mac — AV1 output uses the CPU (SVT-AV1); Apple Silicon can decode AV1 (M3+)
- CPU: always available through SVT-AV1

## When to choose HEVC

Use HEVC when you want most of AV1’s storage benefit with
better playback compatibility across TVs, phones, and
set-top boxes.

Hardware support:

- NVIDIA: Maxwell+
- Intel: 6th gen+
- AMD: Polaris+
- Apple: M1+/T2 and newer
- CPU: x265

## When to choose H.264

Use H.264 when compatibility is the priority and storage
efficiency is secondary.

Hardware support:

- NVIDIA: broadly available
- Intel: broadly available
- AMD: broadly available
- Apple: broadly available
- CPU: x264

## Hardware summary by vendor

| Vendor | AV1 | HEVC | H.264 |
|--------|-----|------|-------|
| NVIDIA | RTX 40 (Ada) | Maxwell+ | Yes |
| Intel | Arc / Meteor Lake+ | 6th gen+ | Yes |
| AMD | RDNA 3+ on compatible driver/FFmpeg stacks | Polaris+ | Yes |
| Apple | CPU only (no HW encoder) | M1+/T2 | Yes |
| CPU | Yes | Yes | Yes |

## BPP

BPP means bits per pixel. It measures how much video data is
being spent per rendered pixel and frame, which makes it
more useful than plain bitrate when you compare files across
different resolutions and frame rates.

Typical ranges:

- `> 0.15`: high quality, usually still worth evaluating
- `~0.10`: medium quality, often already efficient
- `< 0.05`: heavily compressed, likely to look blocky

Alchemist uses BPP because bitrate alone lies. A 4K file and
a 1080p file can share the same bitrate and look completely
different. BPP normalizes for resolution and frame rate, so
the planner can skip files that are already efficiently
compressed instead of re-encoding on guesswork.
