---
title: AMD GPU Setup (VAAPI/AMF)
description: Complete guide to setting up AMD hardware acceleration using VAAPI (Linux) and AMF (Windows).
---

# AMD GPU Setup (VAAPI/AMF)

AMD GPUs support hardware-accelerated encoding through VAAPI on Linux and AMF on Windows. This guide covers setup and optimization for AMD Radeon graphics cards.

## Supported Hardware

Hardware encoding support varies by AMD GPU generation:

| Generation | Codecs | Linux (VAAPI) | Windows (AMF) |
|------------|---------|---------------|---------------|
| **GCN 1.0 (HD 7000)** | H.264 | Limited | Limited |
| **GCN 2.0 (R7/R9 200)** | H.264 | Yes | Yes |
| **GCN 3.0/4.0 (RX 400/500)** | H.264, HEVC | Yes | Yes |
| **RDNA 1 (RX 5000)** | H.264, HEVC | Yes | Yes |
| **RDNA 2 (RX 6000)** | H.264, HEVC | Yes | Yes |
| **RDNA 3 (RX 7000)** | H.264, HEVC, AV1 | Yes | Yes |

### Checking Your Hardware

**Linux (VAAPI):**
```bash
# Check for AMD GPU
lspci | grep -i amd

# Verify VAAPI support
vainfo --display drm --device /dev/dri/renderD128

# Check available encoders
ffmpeg -encoders | grep vaapi
```

**Windows (AMF):**
```bash
# Check FFmpeg AMF encoders
ffmpeg -encoders | grep amf
```

Expected output (Linux):
```
 V....D h264_vaapi          H.264/AVC (VAAPI) (codec h264)
 V....D hevc_vaapi          H.265/HEVC (VAAPI) (codec hevc)
```

Expected output (Windows):
```
 V....D h264_amf            AMD AMF H.264 Encoder (codec h264)
 V....D hevc_amf            AMD AMF HEVC encoder (codec hevc)
```

## Linux Setup (VAAPI)

### Docker Installation

1. **Pass GPU devices:**
   ```yaml
   services:
     alchemist:
       image: ghcr.io/bybrooklyn/alchemist:latest
       devices:
         - /dev/dri:/dev/dri
       group_add:
         - video
         - render
   ```

2. **Verify GPU access:**
   ```bash
   docker exec -it alchemist ls -la /dev/dri/
   # Should show renderD128 or similar
   ```

### Binary Installation

1. **Install AMD drivers:**

   **Ubuntu/Debian:**
   ```bash
   # Add AMD GPU repository
   wget -q -O - https://repo.radeon.com/rocm/rocm.gpg.key | sudo apt-key add -
   echo 'deb [arch=amd64] https://repo.radeon.com/rocm/apt/debian/ focal main' | sudo tee /etc/apt/sources.list.d/rocm.list
   sudo apt update
   
   # Install drivers
   sudo apt install rocm-dkms
   sudo apt install mesa-va-drivers
   ```

   **Fedora/RHEL:**
   ```bash
   sudo dnf install mesa-va-drivers
   sudo dnf install libva-utils
   ```

   **Arch Linux:**
   ```bash
   sudo pacman -S mesa-va-drivers
   sudo pacman -S libva-utils
   ```

2. **Verify VAAPI:**
   ```bash
   vainfo --display drm --device /dev/dri/renderD128
   ```

3. **User permissions:**
   ```bash
   sudo usermod -a -G video $USER
   sudo usermod -a -G render $USER
   # Log out and back in
   ```

## Windows Setup (AMF)

### Requirements

1. **AMD Adrenalin drivers** (latest version recommended)
2. **AMD AMF SDK** (included with drivers)
3. **FFmpeg with AMF support**

### Installation

1. **Download AMD drivers:**
   - Visit [amd.com/support](https://amd.com/support)
   - Download latest Adrenalin drivers
   - Install with "Standard" or "Custom" installation

2. **Verify AMF support:**
   ```cmd
   ffmpeg -encoders | findstr amf
   ```

## Configuration

### In Alchemist

**Linux (VAAPI):**
1. Navigate to **Settings** → **Hardware**
2. Set **Preferred Vendor** to `amd`
3. Set **Device Path** to `/dev/dri/renderD128`
4. Verify "AMD VAAPI" appears in hardware status

**Windows (AMF):**
1. Navigate to **Settings** → **Hardware**  
2. Set **Preferred Vendor** to `amd`
3. Leave **Device Path** empty
4. Verify "AMD AMF" appears in hardware status

### Quality Settings

AMD encoding quality varies by implementation:

**VAAPI Quality (Linux):**
| Profile | Quality Level | Use Case |
|---------|--------------|----------|
| **Quality** | High | Best quality, slower |
| **Balanced** | Medium | Good balance |
| **Speed** | Fast | Faster encoding |

**AMF Quality (Windows):**
| Profile | CRF/Quality | Use Case |
|---------|-------------|----------|
| **Quality** | 20-24 | Archive quality |
| **Balanced** | 25-28 | General use |
| **Speed** | 29-32 | Quick turnaround |

## Troubleshooting

### Linux (VAAPI)

#### "No VAAPI device found"

**Solutions:**

1. **Check GPU detection:**
   ```bash
   lspci | grep -i vga
   dmesg | grep amdgpu
   ```

2. **Verify device nodes:**
   ```bash
   ls -la /dev/dri/
   # Should show renderD128, card0, etc.
   ```

3. **Test VAAPI directly:**
   ```bash
   vainfo --display drm --device /dev/dri/renderD128
   ```

#### "VAAPI initialization failed"

**Solutions:**

1. **Install mesa drivers:**
   ```bash
   sudo apt install mesa-va-drivers libva-dev
   ```

2. **Set environment variables:**
   ```bash
   export LIBVA_DRIVER_NAME=radeonsi
   export LIBVA_DRIVERS_PATH=/usr/lib/x86_64-linux-gnu/dri
   ```

3. **Check user groups:**
   ```bash
   groups $USER
   # Should include 'video' and 'render'
   ```

### Windows (AMF)

#### "AMF encoder not available"

**Solutions:**

1. **Update AMD drivers:**
   - Download latest Adrenalin drivers
   - Use DDU (Display Driver Uninstaller) if needed

2. **Verify GPU detection:**
   ```cmd
   dxdiag
   # Check Display tab for AMD GPU
   ```

3. **Check Windows version:**
   - AMF requires Windows 10 or later
   - Update Windows if necessary

#### Poor quality output

**Solutions:**

1. **Adjust quality settings:**
   ```toml
   [transcode]
   quality_profile = "quality"
   ```

2. **Use constant quality mode:**
   ```toml
   [transcode.encoder_args]
   extra_args = ["-rc", "cqp", "-qp_i", "22", "-qp_p", "24"]
   ```

## Performance Optimization

### Linux Optimization

1. **Enable GPU scheduler:**
   ```bash
   echo 'KERNEL=="card*", SUBSYSTEM=="drm", DRIVERS=="amdgpu", ATTR{device/power_dpm_force_performance_level}="high"' | sudo tee /etc/udev/rules.d/30-amdgpu-pm.rules
   sudo udevadm control --reload-rules
   ```

2. **Optimize for encoding:**
   ```bash
   echo high | sudo tee /sys/class/drm/card*/device/power_dpm_force_performance_level
   ```

### Windows Optimization

1. **AMD Adrenalin settings:**
   - Open AMD Software
   - Graphics → Advanced → GPU Workload → "Compute"
   - Set Power Limit to maximum

2. **Registry optimizations:**
   ```reg
   [HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0000]
   "PP_ThermalAutoThrottlingEnable"=dword:00000000
   ```

### Concurrent Encoding

AMD GPUs generally support fewer concurrent streams than NVIDIA:

```toml
[transcode]
concurrent_jobs = 1  # Start with 1, test higher values
```

## Best Practices

### Codec Selection

#### HEVC (Recommended)
- Best quality/size ratio
- Good AMD hardware support
- Wide compatibility

```toml
[transcode]
output_codec = "hevc"
quality_profile = "balanced"
```

#### H.264 (Maximum compatibility)
- Universal playback support
- Fastest encoding
- Larger file sizes

```toml
[transcode]
output_codec = "h264"
quality_profile = "speed"
```

### Quality Settings

For best results with AMD encoding:

**Linux (VAAPI):**
```toml
[transcode.encoder_args]
extra_args = ["-vaapi_device", "/dev/dri/renderD128", "-qp", "24"]
```

**Windows (AMF):**
```toml
[transcode.encoder_args]
extra_args = ["-usage", "transcoding", "-rc", "cqp", "-qp", "24"]
```

### Thermal Management

AMD GPUs can run hot during extended encoding:

1. **Monitor temperatures:**
   ```bash
   # Linux
   sensors
   
   # Windows - Use MSI Afterburner or AMD Software
   ```

2. **Adjust fan curves** in AMD Software

3. **Consider undervolting** for 24/7 operation

### Power Efficiency

For always-on systems:
- Lower power limits in AMD Software
- Use "Balanced" or "Speed" quality profiles  
- Enable power management features
- Consider concurrent job limits

## Hardware-Specific Notes

### Older AMD Cards (Pre-RDNA)
- Limited to H.264 encoding
- Quality may be lower than modern cards
- Consider CPU fallback for critical content

### APUs (Integrated Graphics)
- Share system memory
- Thermal constraints in compact systems
- Good for low-power applications

### High-end Cards (RX 6000/7000)
- Excellent encoding performance
- Support for modern codecs
- May require adequate cooling