---
title: Troubleshooting
description: Common failure modes and how to isolate them.
---

## No hardware encoder detected

Check **Settings → Hardware → Probe Log** first. The probe
log shows the exact FFmpeg encoder test that failed.

Then verify the platform path:

- NVIDIA: confirm `nvidia-container-toolkit` and `nvidia-smi`
- Intel/AMD on Linux: confirm `/dev/dri` passthrough and
  container group access
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
