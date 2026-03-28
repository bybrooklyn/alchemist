---
title: Hardware Acceleration
description: GPU detection, vendor selection, and fallback behavior.
---

Alchemist detects hardware automatically at startup and
selects the best available encoder. Override in
**Settings → Hardware**.

## Detection order (auto mode)

1. Apple VideoToolbox (macOS only)
2. NVIDIA NVENC (checks `/dev/nvidiactl`)
3. Intel VAAPI, then QSV fallback (checks `/dev/dri/renderD128`)
4. AMD VAAPI (Linux) or AMF (Windows)
5. CPU fallback (SVT-AV1, x265, x264)

## Encoder support by vendor

| Vendor | AV1 | HEVC | H.264 | Notes |
|--------|-----|------|-------|-------|
| NVIDIA NVENC | RTX 30/40 | Maxwell+ | All | Best for speed |
| Intel QSV | 12th gen+ | 6th gen+ | All | Best for power efficiency |
| AMD VAAPI/AMF | RDNA 2+ | Polaris+ | All | Linux VAAPI / Windows AMF |
| Apple VideoToolbox | M3+ | M1+/T2 | All | Binary install recommended |
| CPU | All | All | All | Always available |

## Hardware probe

Alchemist probes each encoder at startup with a test encode.
See results in **Settings → Hardware → Probe Log**. Probe
failures include the FFmpeg stderr explaining why.

## Vendor-specific guides

- [NVIDIA (NVENC)](/hardware/nvidia)
- [Intel (QSV / VAAPI)](/hardware/intel)
- [AMD (VAAPI / AMF)](/hardware/amd)
- [Apple (VideoToolbox)](/hardware/apple)
- [CPU Encoding](/hardware/cpu)
