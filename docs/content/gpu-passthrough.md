---
title: GPU Passthrough
description: Pass NVIDIA GPUs (via nvidia-container-toolkit), Intel iGPUs, and AMD GPUs (via /dev/dri) into the Alchemist Docker container for NVENC, QSV, and VAAPI.
keywords:
  - docker gpu passthrough
  - nvidia container toolkit
  - /dev/dri docker
  - vaapi docker
---

Use `ghcr.io/bybrooklyn/alchemist:latest`. The image bundles
an FFmpeg with VAAPI, QSV, and NVENC encoders built in, so once
the GPU device is visible inside the container, Alchemist detects
and uses it. It also includes `vainfo` for in-container VAAPI
diagnostics.

## Device permissions (`/dev/dri`)

The container runs as **root by default**. Root can open the
`/dev/dri` render nodes regardless of their group, so for Intel
and AMD you only need to pass the device in — **no `group_add`
is required**.

You only need group access when you set `PUID`/`PGID` to run as
an unprivileged user. In that case, **do not** use group *names*
like `render` — those groups don't exist inside the container and
Docker fails to start with:

```
Error response from daemon: unable to find group render: no matching entries in group file
```

Pass the host's **numeric** render group id instead. Find it on
the host:

```bash
getent group render | cut -d: -f3      # e.g. 105
# or, equivalently, the group that owns the render node:
stat -c '%g' /dev/dri/renderD128
```

Then add that id (quote it so YAML keeps it a string):

```yaml
environment:
  - PUID=1000
  - PGID=1000
group_add:
  - "105"   # numeric GID of the host 'render' group
```

Alchemist preserves numeric supplemental groups when it drops
from root to `PUID`/`PGID`, so the process keeps access to the
render node after startup.

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
      - ./config:/app/config
      - ./data:/app/data
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
  -v ./config:/app/config \
  -v ./data:/app/data \
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

Intel passthrough on Linux uses `/dev/dri`. Pass the device into
the container — that's all the default (root) container needs.
Driver auto-detection handles modern Intel iGPUs; only set
`LIBVA_DRIVER_NAME=iHD` if detection picks the wrong driver.
If you run with `PUID`/`PGID`, also add the numeric render GID
(see [Device permissions](#device-permissions-devdri)).

### Docker Compose

```yaml
services:
  alchemist:
    image: ghcr.io/bybrooklyn/alchemist:latest
    ports:
      - "3000:3000"
    volumes:
      - ./config:/app/config
      - ./data:/app/data
      - /path/to/media:/media
    devices:
      - /dev/dri:/dev/dri
    environment:
      - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
      - ALCHEMIST_DB_PATH=/app/data/alchemist.db
      # Optional — only if auto-detection picks the wrong driver:
      # - LIBVA_DRIVER_NAME=iHD
    restart: unless-stopped
```

### docker run

```bash
docker run -d \
  --name alchemist \
  --device /dev/dri:/dev/dri \
  -p 3000:3000 \
  -v ./config:/app/config \
  -v ./data:/app/data \
  -v /path/to/media:/media \
  -e ALCHEMIST_CONFIG_PATH=/app/config/config.toml \
  -e ALCHEMIST_DB_PATH=/app/data/alchemist.db \
  --restart unless-stopped \
  ghcr.io/bybrooklyn/alchemist:latest
```

### Verify

Inside the container:

```bash
vainfo --display drm --device /dev/dri/renderD128
ffmpeg -hide_banner -encoders | grep -E 'vaapi|qsv'
```

`vainfo` should report the Intel VAAPI driver and supported
profiles, and FFmpeg should list `h264_vaapi`, `hevc_vaapi`, and
the `*_qsv` encoders (the bundled FFmpeg ships with them).
Alchemist tries VAAPI first for Intel, then QSV as fallback.

## AMD

AMD on Linux uses the same `/dev/dri` passthrough model as
Intel. Auto-detection usually selects the `radeonsi` driver;
set `LIBVA_DRIVER_NAME=radeonsi` only if it doesn't. As with
Intel, the default (root) container needs no `group_add` — add
the numeric render GID only when running with `PUID`/`PGID`
(see [Device permissions](#device-permissions-devdri)).

### Docker Compose

```yaml
services:
  alchemist:
    image: ghcr.io/bybrooklyn/alchemist:latest
    ports:
      - "3000:3000"
    volumes:
      - ./config:/app/config
      - ./data:/app/data
      - /path/to/media:/media
    devices:
      - /dev/dri:/dev/dri
    environment:
      - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
      - ALCHEMIST_DB_PATH=/app/data/alchemist.db
      # Optional — only if auto-detection picks the wrong driver:
      # - LIBVA_DRIVER_NAME=radeonsi
    restart: unless-stopped
```

### docker run

```bash
docker run -d \
  --name alchemist \
  --device /dev/dri:/dev/dri \
  -p 3000:3000 \
  -v ./config:/app/config \
  -v ./data:/app/data \
  -v /path/to/media:/media \
  -e ALCHEMIST_CONFIG_PATH=/app/config/config.toml \
  -e ALCHEMIST_DB_PATH=/app/data/alchemist.db \
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
| `unable to find group render` on start | `group_add: render` uses a group *name* that doesn't exist inside the container | Drop `group_add` entirely when running as root; for `PUID`/`PGID`, use the host's numeric render GID — see [Device permissions](#device-permissions-devdri) |
| Permission denied on `/dev/dri` | Running unprivileged (`PUID`/`PGID`) without render-group access | Add the numeric render GID via `group_add: ["<gid>"]` (`getent group render | cut -d: -f3`) |
| No encoder found | The GPU was found but FFmpeg couldn't use it (custom/system FFmpeg without the backend) | The bundled FFmpeg ships VAAPI/QSV/NVENC; if you replaced it, check `ffmpeg -encoders` and confirm the host driver/toolkit is installed |
| CPU fallback despite GPU | Device passthrough or toolkit missing, or probe failed | Check **Settings → Hardware → Probe Log**, verify `/dev/dri` or NVIDIA toolkit, then restart the container |
