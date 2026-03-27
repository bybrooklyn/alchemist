---
title: NVIDIA (NVENC) Setup
description: Complete guide to setting up NVIDIA NVENC hardware acceleration with Alchemist.
---

# NVIDIA NVENC Setup

NVIDIA GPUs provide excellent hardware-accelerated encoding via NVENC. This guide covers setup, troubleshooting, and optimization for NVIDIA graphics cards.

## Supported Hardware

NVENC is available on most modern NVIDIA GPUs:

| Generation | Codecs | Notes |
|------------|---------|-------|
| **Pascal (GTX 10-series)** | H.264, HEVC | 2 concurrent streams max |
| **Turing (GTX 16/RTX 20-series)** | H.264, HEVC | 3 concurrent streams, improved quality |
| **Ampere (RTX 30-series)** | H.264, HEVC, AV1 | Best performance, AV1 support |
| **Ada Lovelace (RTX 40-series)** | H.264, HEVC, AV1 | Dual AV1 encoders, best efficiency |

### Checking Your Hardware

Verify NVENC support:
```bash
nvidia-smi
```

Check available encoders in FFmpeg:
```bash
ffmpeg -encoders | grep nvenc
```

Expected output:
```
 V....D av1_nvenc           NVIDIA NVENC av1 encoder (codec av1)
 V....D h264_nvenc          NVIDIA NVENC H.264 encoder (codec h264)
 V....D hevc_nvenc          NVIDIA NVENC hevc encoder (codec hevc)
```

## Installation

### Docker Setup (Recommended)

1. **Install NVIDIA Container Toolkit** on your host:

   **Ubuntu/Debian:**
   ```bash
   distribution=$(. /etc/os-release;echo $ID$VERSION_ID)
   curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
   curl -s -L https://nvidia.github.io/libnvidia-container/$distribution/libnvidia-container.list | \
     sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
     sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list
   sudo apt-get update && sudo apt-get install -y nvidia-container-toolkit
   sudo systemctl restart docker
   ```

   **RHEL/CentOS/Fedora:**
   ```bash
   curl -s -L https://nvidia.github.io/nvidia-docker/centos7/nvidia-docker.repo | \
     sudo tee /etc/yum.repos.d/nvidia-docker.repo
   sudo yum install -y nvidia-container-toolkit
   sudo systemctl restart docker
   ```

2. **Update Docker Compose:**
   ```yaml
   services:
     alchemist:
       image: ghcr.io/bybrooklyn/alchemist:latest
       deploy:
         resources:
           reservations:
             devices:
               - driver: nvidia
                 count: 1
                 capabilities: [gpu]
   ```

3. **Test GPU Access:**
   ```bash
   docker run --rm --gpus all nvidia/cuda:11.0-base nvidia-smi
   ```

### Binary Installation

For binary installations, ensure:
1. **NVIDIA drivers** are installed and up-to-date
2. **CUDA toolkit** (optional, for development)
3. **FFmpeg with NVENC support**

#### Installing FFmpeg with NVENC

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install ffmpeg
# Verify NVENC support
ffmpeg -encoders | grep nvenc
```

**From Source:**
```bash
git clone https://git.ffmpeg.org/ffmpeg.git
cd ffmpeg
./configure --enable-cuda --enable-nvenc --enable-nonfree
make -j$(nproc)
sudo make install
```

## Configuration

### In Alchemist

1. Navigate to **Settings** → **Hardware**
2. Set **Preferred Vendor** to `nvidia`
3. Leave **Device Path** empty (auto-detect)
4. Verify detection in the hardware status section

### Quality Settings

NVENC quality is controlled by presets and CQ (Constant Quality) values:

| Profile | NVENC Preset | CQ Value | Use Case |
|---------|-------------|----------|----------|
| **Quality** | `p7` | 20-24 | Archival, slow encodes |
| **Balanced** | `p4` | 25-28 | General purpose |
| **Speed** | `p1` | 30-35 | Fast turnaround |

## Troubleshooting

### "No NVENC capable devices found"

**Causes:**
- GPU drivers not installed
- Container can't access GPU
- Unsupported GPU model

**Solutions:**

1. **Check drivers:**
   ```bash
   nvidia-smi
   ```

2. **Verify container access:**
   ```bash
   docker run --rm --gpus all nvidia/cuda:11.0-base nvidia-smi
   ```

### "NVENC encoder failed to initialize"

**Common causes:**
- All encode sessions in use
- Insufficient GPU memory
- Driver version mismatch

**Solutions:**

1. **Reduce concurrent jobs:**
   ```toml
   [transcode]
   concurrent_jobs = 1
   ```

2. **Check GPU memory:**
   ```bash
   nvidia-smi
   ```

## Best Practices

1. **Quality Testing**: Always test quality before bulk transcoding
2. **Temperature Monitoring**: Keep GPU temperatures under 83°C
3. **Driver Updates**: Update drivers regularly for bug fixes
4. **Backup Strategy**: Keep originals until quality is verified

## Codec Recommendations

### AV1 (RTX 30/40 series)
- Best compression
- Slower encoding
- Future-proof format
- Ideal for archival

### HEVC
- Excellent compression
- Wide compatibility
- Good encoding speed
- Recommended for most users

### H.264
- Universal compatibility
- Fast encoding
- Larger file sizes
- Good for compatibility requirements