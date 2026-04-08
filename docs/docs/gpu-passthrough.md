---
title: GPU Passthrough
description: Docker GPU passthrough for NVIDIA, Intel, and AMD.
---

Use `ghcr.io/bybrooklyn/alchemist:latest`. Alchemist does
not use `PUID` or `PGID`. Handle device permissions with
Docker device mappings and host groups.

## NVIDIA

Install `nvidia-container-toolkit` on the host before you
start the container.

### Host setup

```bash
distribution=$(. /etc/os-release; echo $ID$VERSION_ID)
curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | \
  sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
curl -s -L "https://nvidia.github.io/libnvidia-container/$distribution/libnvidia-container.list" | \
  sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
  sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list
sudo apt update
sudo apt install -y nvidia-container-toolkit
sudo systemctl restart docker
```

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
    environment:
      - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
      - ALCHEMIST_DB_PATH=/app/data/alchemist.db
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
    restart: unless-stopped
```

### docker run

```bash
docker run -d \
  --name alchemist \
  --gpus all \
  -p 3000:3000 \
  -v ~/.config/alchemist:/app/config \
  -v ~/.config/alchemist:/app/data \
  -v /path/to/media:/media \
  -e ALCHEMIST_CONFIG_PATH=/app/config/config.toml \
  -e ALCHEMIST_DB_PATH=/app/data/alchemist.db \
  --restart unless-stopped \
  ghcr.io/bybrooklyn/alchemist:latest
```

### Verify

On the host:

```bash
nvidia-smi
```

In the container:

```bash
ffmpeg -encoders | grep nvenc
```

You should see `h264_nvenc`, `hevc_nvenc`, and on supported
cards `av1_nvenc`.

## Intel

Intel passthrough on Linux uses `/dev/dri`. Pass the device
into the container and add the `video` and `render` groups.
For modern Intel iGPUs, set `LIBVA_DRIVER_NAME=iHD`.

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

### Verify

Inside the container:

```bash
vainfo --display drm --device /dev/dri/renderD128
```

If the device is exposed correctly, `vainfo` reports the
Intel VAAPI driver and supported profiles. Alchemist tries
VAAPI first for Intel, then QSV as fallback.

## AMD

AMD on Linux uses the same `/dev/dri` passthrough model as
Intel, but the VAAPI driver should be `radeonsi`.

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
      - LIBVA_DRIVER_NAME=radeonsi
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
  -e LIBVA_DRIVER_NAME=radeonsi \
  --restart unless-stopped \
  ghcr.io/bybrooklyn/alchemist:latest
```

### Verify

Inside the container:

```bash
vainfo --display drm --device /dev/dri/renderD128
```

On Windows, AMD uses AMF. No device passthrough is required.
If NVIDIA and Intel are absent, Alchemist uses AMF
automatically when FFmpeg exposes the AMF encoders.

## Troubleshooting

| Problem | Cause | Fix |
|--------|-------|-----|
| Permission denied on `/dev/dri` | Container can see the device but lacks group access | Add `group_add: [video, render]` or the equivalent `--group-add` flags |
| No encoder found | FFmpeg in the container does not expose the backend | Check `ffmpeg -encoders` inside the container; confirm the host driver/toolkit is installed |
| CPU fallback despite GPU | Device passthrough or toolkit missing, or probe failed | Check **Settings → Hardware → Probe Log**, verify `/dev/dri` or NVIDIA toolkit, then restart the container |
