# GPU Passthrough Guide

This guide explains how to enable hardware acceleration for video encoding in Docker containers.

## NVIDIA GPU (NVENC)

### Prerequisites
1. NVIDIA GPU with NVENC support (GTX 1050+ / RTX series / Quadro)
2. NVIDIA drivers installed on host
3. NVIDIA Container Toolkit

### Install NVIDIA Container Toolkit

```bash
# Add NVIDIA package repository
distribution=$(. /etc/os-release;echo $ID$VERSION_ID)
curl -s -L https://nvidia.github.io/nvidia-docker/gpgkey | sudo apt-key add -
curl -s -L https://nvidia.github.io/nvidia-docker/$distribution/nvidia-docker.list | \
  sudo tee /etc/apt/sources.list.d/nvidia-docker.list

# Install toolkit
sudo apt update
sudo apt install -y nvidia-container-toolkit
sudo systemctl restart docker
```

### Docker Compose Configuration

```yaml
services:
  alchemist:
    image: ghcr.io/brooklynloveszelda/alchemist:latest
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
    environment:
      - NVIDIA_VISIBLE_DEVICES=all
```

### Docker CLI

```bash
docker run --gpus all \
  -p 3000:3000 \
  -v /media:/media \
  ghcr.io/brooklynloveszelda/alchemist:latest
```

---

## Intel QuickSync (QSV)

### Prerequisites
1. Intel CPU with integrated graphics (6th Gen+)
2. VAAPI drivers installed on host

### Install VAAPI Drivers (Host)

```bash
# Debian/Ubuntu
sudo apt install intel-media-va-driver-non-free vainfo

# Verify
vainfo
```

### Docker Compose Configuration

```yaml
services:
  alchemist:
    image: ghcr.io/brooklynloveszelda/alchemist:latest
    devices:
      - /dev/dri:/dev/dri
    group_add:
      - video
      - render
    environment:
      - LIBVA_DRIVER_NAME=iHD
```

### Docker CLI

```bash
docker run --device /dev/dri:/dev/dri \
  --group-add video --group-add render \
  -e LIBVA_DRIVER_NAME=iHD \
  -p 3000:3000 \
  -v /media:/media \
  ghcr.io/brooklynloveszelda/alchemist:latest
```

---

## AMD GPU (VAAPI)

### Prerequisites
1. AMD GPU with VAAPI support
2. Mesa VAAPI drivers

### Install Drivers (Host)

```bash
# Debian/Ubuntu
sudo apt install mesa-va-drivers vainfo
```

### Docker Configuration

Same as Intel QSV, but set driver:

```yaml
environment:
  - LIBVA_DRIVER_NAME=radeonsi
```

---

## Verification

After starting the container, check hardware detection in the logs:

```
Selected Hardware: Intel QSV
  Device Path: /dev/dri/renderD128
```

If you see `CPU (Software)`, hardware acceleration is not working.

## Troubleshooting

| Issue | Solution |
|-------|----------|
| `vainfo: error` | Install VAAPI drivers on host |
| `CUDA error` | Install NVIDIA Container Toolkit |
| CPU fallback despite GPU | Check device permissions in container |
| Permission denied on `/dev/dri` | Add `--group-add video --group-add render` |
