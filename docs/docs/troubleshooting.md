---
title: Troubleshooting — GPU Detection, Queue, Skip Decisions
description: Fix common Alchemist issues — NVENC or VAAPI not detected, CPU fallback despite a GPU, jobs stuck in Queued, unexpected skips, and missing VMAF scores.
keywords:
  - nvenc not detected docker
  - vaapi not working
  - gpu not detected ffmpeg
  - jobs stuck queued
---

## No hardware encoder detected

Check **Settings → Hardware → Probe Log** first. The probe
log shows the exact FFmpeg encoder test that failed.

Then verify the platform path:

- NVIDIA: confirm `nvidia-container-toolkit` and `nvidia-smi`
  — full walkthrough in
  [NVENC not detected](/troubleshooting/nvenc-not-detected)
- Intel/AMD on Linux: confirm `/dev/dri` passthrough and
  container group access — full walkthrough in
  [VAAPI not detected](/troubleshooting/vaapi-not-detected)
- Apple: confirm `ffmpeg -encoders | grep videotoolbox`

## Jobs stuck in Queued

The engine may be paused. Check the header state first.

If the engine is already running, check:

- active concurrent limit in **Settings → Runtime**
- schedule window state
- current failures in the logs

## CPU fallback despite GPU

Most cases are one of these:

- `/dev/dri` not passed through on Linux
- `nvidia-container-toolkit` missing for NVIDIA
- probe failure for the requested codec/backend

Check **Settings → Hardware → Probe Log**. If the probe log
shows a backend error, fix that before changing planner
thresholds.

## Files skipped unexpectedly

See [Skip Decisions](/skip-decisions). The common causes are:

- `min_bpp_threshold` is too high
- `size_reduction_threshold` is too high
- the file is below `min_file_size_mb`
- the file is already in the target codec

## VMAF scores not appearing

VMAF is optional. Check that FFmpeg was built with VMAF
support:

```bash
ffmpeg -filters | grep vmaf
```

If nothing matches, VMAF scoring is unavailable in that
FFmpeg build.

## Log locations

- Docker: `docker logs -f alchemist`
- Binary: stdout, or redirect with `./alchemist > alchemist.log 2>&1`
- systemd: `journalctl -u alchemist -f`

## More help

GitHub Issues:
[https://github.com/bybrooklyn/alchemist/issues](https://github.com/bybrooklyn/alchemist/issues)

## Detailed troubleshooting pages

- [NVENC not detected](/troubleshooting/nvenc-not-detected) —
  NVIDIA driver, container toolkit, FFmpeg NVENC, probe log.
- [VAAPI not detected](/troubleshooting/vaapi-not-detected) —
  Intel / AMD on Linux — `/dev/dri`, `vainfo`, render group,
  `LIBVA_DRIVER_NAME`.
- [Jellyfin direct-play failing](/troubleshooting/jellyfin-direct-play-failing) —
  why Jellyfin still transcodes after Alchemist processed
  the file.

## Related pages

- [Hardware Acceleration](/hardware) — vendor-specific setup.
- [GPU Passthrough](/gpu-passthrough) — Docker device and
  group configuration.
- [Skip Decisions](/skip-decisions) — why a file wasn't
  transcoded.
- [Engine Modes](/engine-modes) — concurrency limits and
  draining behavior.
