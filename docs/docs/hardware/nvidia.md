---
title: NVIDIA
description: Configure NVIDIA NVENC for Alchemist. Supported NVENC generations (Pascal through Ada), nvidia-container-toolkit setup, and AV1 NVENC on RTX 30/40.
keywords:
  - nvenc
  - nvenc docker
  - av1 nvenc rtx 40
  - hevc nvenc
---

Alchemist uses NVENC when NVIDIA is available and selected.
Set **Settings → Hardware → Preferred Vendor** to `nvidia`
if you want to pin it instead of using auto detection.

## Supported generations

| Generation | Example cards | H.264 | HEVC | AV1 | Notes |
|-----------|---------------|-------|------|-----|------|
| Pascal | GTX 10-series | Yes | Yes | No | 2 concurrent encode streams on consumer cards |
| Turing | GTX 16 / RTX 20 | Yes | Yes | No | Better quality than Pascal |
| Ampere | RTX 30 | Yes | Yes | Yes | First NVENC generation with AV1 |
| Ada Lovelace | RTX 40 | Yes | Yes | Yes | Dual AV1 encoders on supported SKUs |

## Docker

Install `nvidia-container-toolkit` on the host first.

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

## Binary installs

Verify the driver first:

```bash
nvidia-smi
```

Then verify FFmpeg exposes NVENC:

```bash
ffmpeg -encoders | grep nvenc
```

Expected encoders:

- `h264_nvenc`
- `hevc_nvenc`
- `av1_nvenc` on RTX 30/40 class hardware

## In Alchemist

Set **Settings → Hardware → Preferred Vendor → nvidia**.
Leave **Device Path** empty. NVENC is detected from the
driver and `/dev/nvidiactl`.

## Troubleshooting

| Problem | Cause | Fix |
|--------|-------|-----|
| No encoder found | FFmpeg lacks NVENC or the driver is missing | Run `ffmpeg -encoders | grep nvenc` and `nvidia-smi`; update driver or container runtime |
| Container cannot see GPU | NVIDIA runtime/toolkit not installed or `--gpus all` missing | Reinstall `nvidia-container-toolkit`, restart Docker, and test with `docker run --rm --gpus all nvidia/cuda:12.0.0-base-ubuntu22.04 nvidia-smi` |
| Driver version mismatch | Host driver and container runtime stack disagree | Update the NVIDIA driver and toolkit together, then restart Docker |
