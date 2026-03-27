---
title: Intel QSV Setup  
description: Complete guide to setting up Intel Quick Sync Video (QSV) hardware acceleration.
---

# Intel Quick Sync Video (QSV) Setup

Intel Quick Sync Video provides excellent hardware acceleration with low power consumption. Available on most Intel CPUs with integrated graphics since Sandy Bridge (2011).

## Supported Hardware

QSV is available on Intel processors with integrated graphics:

| Generation | Codecs | Performance |
|------------|---------|------------|
| **Sandy Bridge (2nd gen)** | H.264 | Basic support |
| **Ivy Bridge (3rd gen)** | H.264 | Improved quality |
| **Haswell (4th gen)** | H.264 | Better efficiency |
| **Broadwell (5th gen)** | H.264, HEVC (decode only) | Low power |
| **Skylake (6th gen)** | H.264, HEVC | HEVC encoding support |
| **Kaby Lake (7th gen)** | H.264, HEVC | Enhanced quality |
| **Coffee Lake (8th-10th gen)** | H.264, HEVC | Improved performance |
| **Tiger Lake (11th gen)** | H.264, HEVC, AV1 (decode) | AV1 hardware decode |
| **Alder Lake (12th gen)** | H.264, HEVC, AV1 | Full AV1 encode/decode |
| **Raptor Lake (13th gen)** | H.264, HEVC, AV1 | Enhanced AV1 performance |

### Checking Your Hardware

Verify QSV support:
```bash
# Check for Intel GPU
lspci | grep -i intel

# Look for iGPU device files
ls -la /dev/dri/

# Check FFmpeg QSV encoders
ffmpeg -encoders | grep qsv
```

Expected output:
```
 V....D av1_qsv             Intel AV1 encoder (Intel Quick Sync Video acceleration) (codec av1)
 V....D h264_qsv            Intel H.264 encoder (Intel Quick Sync Video acceleration) (codec h264)
 V....D hevc_qsv            Intel HEVC encoder (Intel Quick Sync Video acceleration) (codec hevc)
```

## Installation

### Docker Setup (Recommended)

1. **Pass GPU devices to container:**
   ```yaml
   services:
     alchemist:
       image: ghcr.io/bybrooklyn/alchemist:latest
       devices:
         - /dev/dri:/dev/dri
       group_add:
         - video  # or render group
   ```

2. **Verify access inside container:**
   ```bash
   docker exec -it alchemist ls -la /dev/dri/
   ```

   Should show devices like:
   ```
   renderD128
   card0
   ```

### Binary Installation

1. **Install Intel GPU drivers:**

   **Ubuntu/Debian:**
   ```bash
   # Intel GPU drivers
   sudo apt install intel-media-va-driver-non-free
   sudo apt install vainfo
   
   # Verify VAAPI support
   vainfo
   ```

   **Fedora/RHEL:**
   ```bash
   sudo dnf install intel-media-driver
   sudo dnf install libva-utils
   vainfo
   ```

   **Arch Linux:**
   ```bash
   sudo pacman -S intel-media-driver
   sudo pacman -S libva-utils
   vainfo
   ```

2. **Install FFmpeg with QSV:**
   ```bash
   # Ubuntu/Debian
   sudo apt install ffmpeg
   
   # Verify QSV support
   ffmpeg -encoders | grep qsv
   ```

3. **User permissions:**
   ```bash
   # Add user to video/render group
   sudo usermod -a -G video $USER
   sudo usermod -a -G render $USER
   # Log out and back in
   ```

## Configuration

### In Alchemist

1. Navigate to **Settings** → **Hardware**
2. Set **Preferred Vendor** to `intel`
3. Set **Device Path** to `/dev/dri/renderD128` (or auto-detect)
4. Verify detection shows "Intel QSV"

### Quality Settings

QSV uses global quality values (lower = better quality):

| Profile | Quality Value | Use Case |
|---------|--------------|----------|
| **Quality** | 20 | Best quality, slower |
| **Balanced** | 25 | Good balance |
| **Speed** | 30 | Faster encoding |

### Advanced Configuration

```toml
[hardware]
preferred_vendor = "intel"
device_path = "/dev/dri/renderD128"

[transcode]
quality_profile = "balanced"
output_codec = "hevc"
```

## Troubleshooting

### "No QSV capable devices found"

**Solutions:**

1. **Check iGPU is enabled in BIOS:**
   - Enable "Intel Graphics" or "Internal Graphics"
   - Set "Primary Display" to "Auto" or "Intel"

2. **Verify device nodes:**
   ```bash
   ls -la /dev/dri/
   stat /dev/dri/renderD128
   ```

3. **Check user permissions:**
   ```bash
   groups $USER
   # Should include 'video' or 'render'
   ```

### "VAAPI initialization failed"

**Solutions:**

1. **Install VAAPI drivers:**
   ```bash
   # Ubuntu/Debian
   sudo apt install i965-va-driver intel-media-va-driver-non-free
   
   # Test VAAPI
   vainfo --display drm --device /dev/dri/renderD128
   ```

2. **Check environment variables:**
   ```bash
   export LIBVA_DRIVER_NAME=iHD  # or i965 for older hardware
   export LIBVA_DRIVERS_PATH=/usr/lib/x86_64-linux-gnu/dri
   ```

### Poor Performance

**Solutions:**

1. **Enable look-ahead:**
   ```toml
   [transcode.encoder_args]
   extra_args = ["-look_ahead", "1"]
   ```

2. **Adjust quality:**
   ```toml
   [transcode.encoder_args]
   global_quality = "23"  # Lower for better quality
   ```

3. **Check thermal throttling:**
   ```bash
   # Monitor CPU/GPU temperatures
   sensors
   ```

### Quality Issues

**Solutions:**

1. **Use higher quality settings:**
   ```toml
   [transcode]
   quality_profile = "quality"
   ```

2. **Enable B-frames:**
   ```toml
   [transcode.encoder_args]
   extra_args = ["-bf", "3", "-b_strategy", "1"]
   ```

## Performance Optimization

### Power Efficiency

Intel QSV excels at power-efficient encoding:

- **Ultra-low power**: Perfect for NAS/always-on systems
- **Thermal management**: Runs cooler than dedicated GPUs
- **Concurrent streams**: Most iGPUs support 2-3 simultaneous encodes

### Memory Usage

Intel iGPU shares system RAM:

```toml
[transcode]
concurrent_jobs = 2  # Safe for most systems
threads = 4          # Reasonable CPU usage
```

### Quality Tuning

For best quality with QSV:

```toml
[transcode.encoder_args]
# HEVC-specific optimizations
extra_args = [
  "-global_quality", "22",
  "-look_ahead", "1", 
  "-bf", "3",
  "-refs", "3"
]
```

## Best Practices

### Hardware Selection

- **Dedicated GPU slot**: Keep iGPU enabled even with dedicated GPU
- **Memory allocation**: Ensure adequate RAM for shared graphics
- **BIOS settings**: Enable iGPU for maximum compatibility

### Operating System

- **Linux**: Best support, low overhead
- **Windows**: Good support with Intel drivers
- **Headless operation**: Works without monitor connected

### Codec Selection

#### AV1 (12th gen+)
```toml
[transcode]
output_codec = "av1"
quality_profile = "quality"  # AV1 benefits from higher quality settings
```

#### HEVC (6th gen+) 
```toml
[transcode]
output_codec = "hevc"
quality_profile = "balanced"  # Good balance of speed/quality
```

#### H.264 (All generations)
```toml
[transcode]
output_codec = "h264"
quality_profile = "speed"  # H.264 encodes quickly
```

## Hardware-Specific Notes

### NUCs and Mini PCs
- Excellent for dedicated transcoding appliances
- Low power consumption
- Passive cooling options available

### Server CPUs
- Xeon processors often lack iGPU
- Check specifications before purchase
- Consider discrete GPU for servers

### Laptops
- May have power/thermal limitations  
- Consider reducing concurrent jobs
- Monitor temperatures during extended use