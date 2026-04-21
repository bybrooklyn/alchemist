---
title: Intel Quick Sync and VAAPI Setup
description: Configure Intel Quick Sync (QSV) and VAAPI for Alchemist. Driver install, /dev/dri passthrough, and Intel Arc notes. AV1 on 12th-generation and newer.
keywords:
  - intel quick sync
  - qsv ffmpeg
  - intel vaapi
  - intel arc transcoding
---

Alchemist tries Intel VAAPI first and QSV second. That
matches current Intel Linux reality: Intel Arc uses VAAPI
through the `i915` or `xe` driver, not QSV. QSV remains a
fallback path for older Intel hardware and FFmpeg setups.

## Supported generations

| Generation | H.264 | HEVC | AV1 | Notes |
|-----------|-------|------|-----|------|
| Intel iGPU, all supported generations | Yes | 6th gen+ | 12th gen+ | Alchemist prefers VAAPI, then QSV |

## Critical note for Intel Arc

Intel Arc uses VAAPI via the Linux DRM stack. Do not force
QSV for Arc unless you have a specific reason and have
verified it works in your FFmpeg build. The expected path is
VAAPI first.

## Host setup

Install the Intel VAAPI driver and verification tools.

```bash
sudo apt install intel-media-va-driver-non-free vainfo
```

Verify the render node directly on the host:

```bash
vainfo --display drm --device /dev/dri/renderD128
```

For newer systems, `renderD129` may be the Intel node. Check
`ls -l /dev/dri` if `renderD128` is not Intel.

## Docker

Pass `/dev/dri` into the container and add the `video` and
`render` groups. Set `LIBVA_DRIVER_NAME=iHD` for modern
Intel media drivers.

### Docker Compose

```yaml
services:
  alchemist:
    image: ghcr.io/bybrooklyn/alchemist:latest
    ports:
      - "3000:3000"
    volumes:
      - ~/.config/alchemist:/app/config
      - ~/.config/alchemist:/app/data
      - /path/to/media:/media
    devices:
      - /dev/dri:/dev/dri
    group_add:
      - video
      - render
    environment:
      - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
      - ALCHEMIST_DB_PATH=/app/data/alchemist.db
      - LIBVA_DRIVER_NAME=iHD
    restart: unless-stopped
```

### docker run

```bash
docker run -d \
  --name alchemist \
  --device /dev/dri:/dev/dri \
  --group-add video \
  --group-add render \
  -p 3000:3000 \
  -v ~/.config/alchemist:/app/config \
  -v ~/.config/alchemist:/app/data \
  -v /path/to/media:/media \
  -e ALCHEMIST_CONFIG_PATH=/app/config/config.toml \
  -e ALCHEMIST_DB_PATH=/app/data/alchemist.db \
  -e LIBVA_DRIVER_NAME=iHD \
  --restart unless-stopped \
  ghcr.io/bybrooklyn/alchemist:latest
```

## Verify inside the container

```bash
vainfo --display drm --device /dev/dri/renderD128
ffmpeg -encoders | grep -E 'vaapi|qsv'
```

If VAAPI is healthy, Alchemist should probe `av1_vaapi`,
`hevc_vaapi`, and `h264_vaapi` first, then the QSV encoders.

## In Alchemist

Set **Settings → Hardware → Preferred Vendor → intel**.
Only set **Device Path** if auto detection chooses the wrong
render node on a multi-GPU Linux host.
