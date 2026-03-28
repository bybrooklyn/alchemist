# Apple VideoToolbox Setup

Complete guide to setting up Apple VideoToolbox hardware acceleration on macOS.

# Apple VideoToolbox Setup

Apple VideoToolbox provides hardware-accelerated encoding on macOS using built-in media engines in Apple Silicon and Intel Macs. This is the most efficient option for macOS users.

## Supported Hardware

VideoToolbox support varies by Mac model and codec:

| Hardware | H.264 | HEVC | AV1 | Notes |
|----------|-------|------|-----|-------|
| **Intel Macs (2016+)** | ✅ | ✅ | ❌ | Requires T2 chip for HEVC |
| **Apple Silicon M1** | ✅ | ✅ | ❌ | Dedicated media engines |
| **Apple Silicon M2** | ✅ | ✅ | ❌ | Enhanced media engines |
| **Apple Silicon M3** | ✅ | ✅ | ✅ | AV1 encode/decode support |

### Checking Your Hardware

Verify VideoToolbox support:
```bash
# Check available encoders
ffmpeg -encoders | grep videotoolbox

# System information
system_profiler SPHardwareDataType
```

Expected output:
```
 V....D h264_videotoolbox    VideoToolbox H.264 Encoder (codec h264)
 V....D hevc_videotoolbox    VideoToolbox H.265 Encoder (codec hevc)
```

On M3 Macs:
```
 V....D av1_videotoolbox     VideoToolbox AV1 Encoder (codec av1)
```

## Installation

### Docker Setup

Running Docker on macOS with VideoToolbox requires special configuration:

```yaml
version: '3.8'
services:
  alchemist:
    image: ghcr.io/bybrooklyn/alchemist:latest
    container_name: alchemist
    ports:
      - "3000:3000"
    volumes:
      - ./config:/app/config
      - ./data:/app/data
      - /path/to/media:/media
    environment:
      - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
      - ALCHEMIST_DB_PATH=/app/data/alchemist.db
    # VideoToolbox access from container is limited
    # Binary installation recommended for best results
```

⚠️ **Note**: Docker containers on macOS have limited access to VideoToolbox. Binary installation is recommended for optimal performance.

### Binary Installation (Recommended)

1. **Download Alchemist binary** for macOS from [GitHub Releases](https://github.com/bybrooklyn/alchemist/releases)

2. **Install FFmpeg with VideoToolbox:**
   ```bash
   # Using Homebrew (recommended)
   brew install ffmpeg
   
   # Verify VideoToolbox support
   ffmpeg -encoders | grep videotoolbox
   ```

3. **Run Alchemist:**
   ```bash
   chmod +x alchemist-macos
   ./alchemist-macos
   ```

### Build from Source

For the latest features:
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/bybrooklyn/alchemist.git
cd alchemist
cargo build --release

./target/release/alchemist
```

## Configuration

### In Alchemist

1. Navigate to **Settings** → **Hardware**
2. Set **Preferred Vendor** to `apple`
3. Leave **Device Path** empty (not applicable)
4. Verify "Apple VideoToolbox" appears in hardware status

### Quality Settings

VideoToolbox uses quality values (higher = better quality):

| Profile | Quality Value | Use Case |
|---------|--------------|----------|
| **Quality** | 55 | Best quality, larger files |
| **Balanced** | 65 | Good balance |
| **Speed** | 75 | Faster encoding, smaller quality |

### Codec-Specific Configuration

#### HEVC (Recommended)
```toml
[transcode]
output_codec = "hevc"
quality_profile = "balanced"

[hardware]
preferred_vendor = "apple"
```

#### H.264 (Maximum compatibility)
```toml
[transcode]
output_codec = "h264"
quality_profile = "speed"  # H.264 encodes quickly
```

#### AV1 (M3 only)
```toml
[transcode]
output_codec = "av1"
quality_profile = "quality"  # AV1 benefits from higher quality
```

## Troubleshooting

### "VideoToolbox encoder not available"

**Solutions:**

1. **Check macOS version:**
   - macOS 10.13+ required for HEVC
   - macOS 14+ required for AV1 (M3 only)

2. **Verify hardware support:**
   ```bash
   system_profiler SPHardwareDataType | grep "Model Identifier"
   ```

3. **Test FFmpeg directly:**
   ```bash
   ffmpeg -f lavfi -i color=c=black:s=64x64:d=0.1 \
     -vf format=yuv420p \
     -c:v hevc_videotoolbox \
     -allow_sw 1 \
     -frames:v 1 -f null -
   ```

### Poor quality output

**Solutions:**

1. **Adjust quality settings:**
   ```toml
   [transcode]
   quality_profile = "quality"
   ```

2. **Use lower quality values (better quality):**
   ```toml
   [transcode.encoder_args]
   extra_args = ["-q:v", "50"]  # Lower = better quality
   ```

3. **Enable constant quality mode:**
   ```toml
   [transcode.encoder_args]
   extra_args = ["-b:v", "0"]  # Forces constant quality
   ```

### Slow encoding performance

**Solutions:**

1. **Check thermal throttling:**
   ```bash
   # Monitor CPU temperature
   sudo powermetrics --samplers smc -n 1
   ```

2. **Adjust concurrent jobs:**
   ```toml
   [transcode]
   concurrent_jobs = 1  # Start with 1 on MacBooks
   ```

3. **Optimize for battery/thermal:**
   ```toml
   [transcode]
   quality_profile = "speed"
   threads = 4  # Limit CPU usage
   ```

## Performance Optimization

### Apple Silicon Optimization

Apple Silicon Macs have dedicated media engines:

```toml
[transcode]
concurrent_jobs = 2  # M1/M2 can handle 2 concurrent streams
quality_profile = "balanced"
```

**M3 Macs** with enhanced engines:
```toml
[transcode]
concurrent_jobs = 3  # M3 Pro/Max can handle more
output_codec = "av1"  # Take advantage of AV1 support
```

### Intel Mac Optimization

Intel Macs rely on CPU + T2 chip:

```toml
[transcode]
concurrent_jobs = 1  # Conservative for thermal management
threads = 8  # Use available CPU cores
quality_profile = "balanced"
```

### Battery Life (MacBooks)

For better battery life during encoding:

```toml
[transcode]
quality_profile = "speed"
concurrent_jobs = 1
threads = 4

[schedule]
# Only encode when plugged in
[[schedule.windows]]
start_time = "22:00"
end_time = "06:00"
enabled = true
```

## Best Practices

### Thermal Management

**MacBooks** (especially Intel models) can throttle during extended encoding:

1. **Monitor temperatures:**
   ```bash
   # Install temperature monitoring
   brew install stats
   ```

2. **Use clamshell mode** when possible (better cooling)

3. **External cooling** for extended sessions

4. **Lower quality profiles** for bulk operations

### Power Management

**Battery considerations:**
- Use "Speed" profile on battery
- Schedule encoding for AC power
- Monitor battery usage in Activity Monitor

**Desktop Macs:**
- Can sustain higher workloads
- Better thermal management
- Support for longer concurrent jobs

### Codec Selection

#### For M3 Macs (AV1 support)
```toml
[transcode]
output_codec = "av1"
quality_profile = "quality"
# Best compression, future-proof
```

#### For M1/M2 Macs
```toml
[transcode]
output_codec = "hevc"
quality_profile = "balanced"  
# Excellent efficiency, wide support
```

#### For older Intel Macs
```toml
[transcode]
output_codec = "h264"
quality_profile = "speed"
# Most compatible, least thermal stress
```

### Quality vs. Speed

**Archive quality** (slow but excellent):
```toml
[transcode]
quality_profile = "quality"
concurrent_jobs = 1

[transcode.encoder_args]
extra_args = ["-q:v", "45"]
```

**Balanced performance** (recommended):
```toml
[transcode]
quality_profile = "balanced"
concurrent_jobs = 2  # Apple Silicon only
```

**Fast turnaround** (quick results):
```toml
[transcode]
quality_profile = "speed"
concurrent_jobs = 1

[transcode.encoder_args]  
extra_args = ["-q:v", "75"]
```

## Hardware-Specific Notes

### MacBook Air
- **Fanless design** limits sustained performance
- Use conservative settings for long encodes
- Monitor thermal throttling

### MacBook Pro
- **Better cooling** supports higher workloads
- 14"/16" models handle concurrent jobs better
- Intel models may need thermal management

### Mac Studio/Pro
- **Excellent cooling** for sustained workloads
- Can handle maximum concurrent jobs
- Ideal for bulk transcoding operations

### Mac mini
- **Good performance** but compact thermal design
- Monitor temperatures during heavy use
- Balance between performance and heat
