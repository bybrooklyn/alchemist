---
title: Hardware Acceleration
description: How Alchemist detects and selects GPU encoders. Per-vendor setup for NVIDIA NVENC, Intel Quick Sync / VAAPI, AMD VAAPI and AMF, and Apple VideoToolbox, with CPU fallback.
keywords:
  - nvenc
  - quick sync
  - vaapi
  - amd amf
  - videotoolbox
  - gpu accelerated transcoding
---

Alchemist detects hardware automatically, actively probes
every plausible backend/codec candidate, and selects a
single active device/backend with a deterministic scoring
policy. Override in **Settings → Hardware**.

After the first successful probe, Alchemist stores a hardware
detection cache keyed by OS, architecture, FFmpeg/FFprobe
versions, hardware settings, and cache schema version. If
that fingerprint still matches on the next boot, the server
can start with the cached backend immediately while a full
probe refreshes runtime state when needed.

## Detection flow (auto mode)

1. Discover plausible candidates
   - Apple VideoToolbox on macOS
   - NVIDIA NVENC when NVIDIA is present
   - Intel / AMD render nodes on Linux via `/sys/class/drm/renderD*`
   - AMD AMF on Windows
2. Actively probe every candidate encoder with a short
   FFmpeg test encode
3. Group successful probes by device path / vendor
4. Choose one active device/backend using codec coverage,
   backend preference, and stable vendor ordering
5. Fall back to CPU only if no GPU probe succeeds and CPU
   fallback is enabled

## Encoder support by vendor

| Vendor | AV1 | HEVC | H.264 | Notes |
|--------|-----|------|-------|-------|
| NVIDIA NVENC | RTX 40 (Ada) | Maxwell+ | All | Best for speed; RTX 30 (Ampere) decodes AV1 but has no AV1 encoder |
| Intel QSV | Arc / Meteor Lake+ | 6th gen+ | All | Best for power efficiency; pre-Arc / pre-Meteor-Lake iGPUs have no AV1 encoder |
| AMD VAAPI/AMF | RDNA 3+ on compatible driver/FFmpeg stacks | Polaris+ | All | Linux VAAPI / Windows AMF; HEVC/H.264 are the validated AMD paths for `0.3.0`. RDNA 2 decodes AV1 but has no AV1 encoder |
| Apple VideoToolbox | None (CPU only) | M1+/T2 | All | No Mac chip has an AV1 hardware encoder; AV1 output uses the CPU (SVT-AV1) path. Apple Silicon can decode AV1 (M3+). Binary install recommended |
| CPU | All | All | All | Always available |

## Hardware probe

Alchemist probes each encoder at startup with a test encode
using a standardized `256x256` lavfi input.

See results in **Settings → Hardware → Probe Log**. The UI
shows:

- the selected device/backend reason
- probe counts (attempted / succeeded / failed)
- per-probe summaries for success and failure
- full FFmpeg stderr for failed probes

On Linux, explicit device paths only apply to render-node
backends such as VAAPI and QSV.

## Cache invalidation

The hardware cache is ignored and rebuilt when any of these
change:

- operating system or architecture
- FFmpeg or FFprobe version marker
- preferred vendor, device path, CPU fallback, or CPU
  encoding setting
- Alchemist's hardware detection cache version

Changing hardware settings in the UI refreshes runtime
hardware state and writes a new cache entry.

## Vendor-specific guides

- [NVIDIA (NVENC)](/hardware/nvidia)
- [Intel (QSV / VAAPI)](/hardware/intel)
- [AMD (VAAPI / AMF)](/hardware/amd)
- [Apple (VideoToolbox)](/hardware/apple)
- [CPU Encoding](/hardware/cpu)

For Docker GPU passthrough (driver toolkit, `/dev/dri`, group
permissions), see [GPU Passthrough](/gpu-passthrough). If your
GPU isn't detected after setup, start with
[Troubleshooting](/troubleshooting).
