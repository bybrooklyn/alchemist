---
title: AMD VAAPI / AMF
description: AMD GPU setup for Linux VAAPI and Windows AMF.
---

AMD uses VAAPI on Linux and AMF on Windows. Set
**Settings → Hardware → Preferred Vendor** to `amd` if you
want to pin it instead of using auto detection.

AMD AV1 remains driver- and FFmpeg-stack-dependent and is
not part of the validated `0.3.0` support matrix. HEVC and
H.264 are the recommended AMD paths for `0.3.0`.

## Supported hardware

| Codec | Support |
|------|---------|
| H.264 | Polaris+ |
| HEVC | Polaris+ |
| AV1 | RDNA 2+ on compatible driver/FFmpeg stacks; available but not part of the validated `0.3.0` support matrix |

## Linux

### Host setup

Install the Mesa VAAPI drivers and verification tool:

```bash
sudo apt install mesa-va-drivers vainfo
```

Verify the render node on the host:

```bash
vainfo --display drm --device /dev/dri/renderD128
```

### Docker

Pass `/dev/dri` into the container and set the AMD VAAPI
driver name to `radeonsi`.

```yaml
services:
  alchemist:
    image: ghcr.io/bybrooklyn/alchemist:latest
    ports:
      - "3000:3000"
    volumes:
      - /path/to/config:/app/config
      - /path/to/data:/app/data
      - /path/to/media:/media
    devices:
      - /dev/dri:/dev/dri
    group_add:
      - video
      - render
    environment:
      - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
      - ALCHEMIST_DB_PATH=/app/data/alchemist.db
      - LIBVA_DRIVER_NAME=radeonsi
    restart: unless-stopped
```

`docker run` equivalent:

```bash
docker run -d \
  --name alchemist \
  --device /dev/dri:/dev/dri \
  --group-add video \
  --group-add render \
  -p 3000:3000 \
  -v /path/to/config:/app/config \
  -v /path/to/data:/app/data \
  -v /path/to/media:/media \
  -e ALCHEMIST_CONFIG_PATH=/app/config/config.toml \
  -e ALCHEMIST_DB_PATH=/app/data/alchemist.db \
  -e LIBVA_DRIVER_NAME=radeonsi \
  --restart unless-stopped \
  ghcr.io/bybrooklyn/alchemist:latest
```

### Verify

```bash
vainfo --display drm --device /dev/dri/renderD128
ffmpeg -encoders | grep vaapi
```

## Windows

Windows AMD support uses AMF. No device passthrough is
required. Install current AMD graphics drivers and confirm
FFmpeg exposes the AMF encoders:

```powershell
ffmpeg -encoders | findstr amf
```

If NVIDIA and Intel are absent, Alchemist uses AMF
automatically when AMF probing succeeds.

## In Alchemist

Set **Settings → Hardware → Preferred Vendor → amd**.
On Linux, only set **Device Path** if you need to force a
specific render node.
