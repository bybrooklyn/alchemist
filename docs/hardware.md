---
title: Hardware Acceleration
description: GPU detection, vendor selection, and fallback behavior.
---

Alchemist detects hardware automatically at startup,
actively probes every plausible backend/codec candidate,
and selects a single active device/backend with a
deterministic scoring policy. Override in
**Settings → Hardware**.

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
| NVIDIA NVENC | RTX 30/40 | Maxwell+ | All | Best for speed |
| Intel QSV | 12th gen+ | 6th gen+ | All | Best for power efficiency |
| AMD VAAPI/AMF | RDNA 2+ | Polaris+ | All | Linux VAAPI / Windows AMF |
| Apple VideoToolbox | M3+ | M1+/T2 | All | Binary install recommended |
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

## Vendor-specific guides

- [NVIDIA (NVENC)](/hardware/nvidia)
- [Intel (QSV / VAAPI)](/hardware/intel)
- [AMD (VAAPI / AMF)](/hardware/amd)
- [Apple (VideoToolbox)](/hardware/apple)
- [CPU Encoding](/hardware/cpu)
