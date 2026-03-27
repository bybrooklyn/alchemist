---
title: CPU Encoding (Software Fallback)
description: Guide to CPU-based software encoding when hardware acceleration is unavailable.
---

# CPU Encoding (Software Fallback)

When hardware acceleration isn't available or enabled, Alchemist falls back to CPU-based software encoding. While slower than GPU acceleration, modern CPUs can produce excellent quality results.

## Supported Encoders

Alchemist uses these high-quality software encoders:

| Codec | Encoder | Quality | Speed | Use Case |
|-------|---------|---------|-------|----------|
| **AV1** | SVT-AV1 | Excellent | Medium | Future-proof archival |
| **AV1** | libaom-av1 | Best | Slow | Maximum quality |
| **HEVC** | x265 | Excellent | Medium | General purpose |
| **H.264** | x264 | Very Good | Fast | Compatibility |

### Checking CPU Encoders

Verify available software encoders:
```bash
ffmpeg -encoders | grep -E "libsvtav1|libaom|libx265|libx264"
```

Expected output:
```
 V....D libsvtav1           SVT-AV1(Scalable Video Technology for AV1) encoder (codec av1)
 V....D libaom-av1          libaom AV1 (codec av1)
 V....D libx265             libx265 H.265 / HEVC (codec hevc)
 V....D libx264             libx264 H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10 (codec h264)
```

## Configuration

### Enabling CPU Encoding

```toml
[hardware]
preferred_vendor = "cpu"  # Force CPU encoding
allow_cpu_encoding = true
cpu_preset = "medium"

[transcode]
output_codec = "av1"  # Recommended for CPU encoding
quality_profile = "balanced"
```

### CPU Presets

CPU presets balance encoding speed vs. quality:

| Preset | SVT-AV1 | x265 | x264 | Use Case |
|--------|---------|------|------|----------|
| **Slow** | 4 | slow | slow | Maximum quality, archival |
| **Medium** | 8 | medium | medium | Balanced performance |
| **Fast** | 12 | fast | fast | Quick turnaround |
| **Faster** | 13 | faster | faster | Speed priority |

### Quality Settings

Quality profiles adjust CRF (Constant Rate Factor) values:

| Profile | AV1 CRF | HEVC CRF | H.264 CRF | File Size | Quality |
|---------|---------|----------|-----------|-----------|---------|
| **Quality** | 24 | 22 | 20 | Larger | Best |
| **Balanced** | 28 | 26 | 23 | Medium | Good |
| **Speed** | 32 | 30 | 26 | Smaller | Acceptable |

### Thread Configuration

Optimize CPU thread usage:

```toml
[transcode]
threads = 0  # Auto-detect (recommended)
# Or set manually:
# threads = 8  # Use 8 threads per job

concurrent_jobs = 1  # Start with 1, increase carefully
```

## Performance Optimization

### Thread Allocation

**Rule of thumb**: Total threads = cores × concurrent_jobs

| CPU Cores | Suggested Config |
|-----------|------------------|
| **4 cores** | 1 job, 4 threads |
| **8 cores** | 1 job, 8 threads or 2 jobs, 4 threads each |
| **16 cores** | 2 jobs, 8 threads each |
| **32+ cores** | 4 jobs, 8 threads each |

```toml
# Example: 16-core CPU
[transcode]
concurrent_jobs = 2
threads = 8
```

### Memory Considerations

Software encoding is memory-intensive:

| Resolution | Recommended RAM per Job |
|------------|------------------------|
| **1080p** | 4-6 GB |
| **1440p** | 6-8 GB |
| **4K** | 8-12 GB |

```toml
# Adjust jobs based on available RAM
[transcode]
concurrent_jobs = 1  # Conservative for 16GB systems
```

### Codec-Specific Optimization

#### AV1 (SVT-AV1) - Recommended
Best compression efficiency for CPU encoding:

```toml
[transcode]
output_codec = "av1"
quality_profile = "balanced"

[transcode.encoder_args]
extra_args = [
  "-preset", "8",      # Good speed/quality balance
  "-crf", "28",        # Quality level
  "-svtav1-params", "tune=0:enable-overlays=1"
]
```

#### HEVC (x265)
Good balance of quality and compatibility:

```toml
[transcode]
output_codec = "hevc"
quality_profile = "balanced"

[transcode.encoder_args]
extra_args = [
  "-preset", "medium",
  "-crf", "26",
  "-x265-params", "log-level=error"
]
```

#### H.264 (x264)
Fastest software encoding:

```toml
[transcode]
output_codec = "h264" 
quality_profile = "speed"

[transcode.encoder_args]
extra_args = [
  "-preset", "fast",
  "-crf", "23"
]
```

## Advanced Configuration

### Two-Pass Encoding

For maximum quality (much slower):

```toml
[transcode.encoder_args]
# AV1 two-pass
extra_args = [
  "-pass", "1", "-an", "-f", "null", "/dev/null", "&&",
  "-pass", "2"
]
```

### Quality-Based Encoding

Use different quality for different content:

```toml
# High quality for movies
[profiles.movies]
quality_profile = "quality"
output_codec = "av1"

# Faster for TV shows
[profiles.tv]
quality_profile = "speed"
output_codec = "hevc"
```

### Grain Synthesis (AV1)

Preserve film grain efficiently:

```toml
[transcode.encoder_args]
extra_args = ["-svtav1-params", "film-grain=50"]
```

## Troubleshooting

### High CPU Usage

**Solutions:**

1. **Reduce concurrent jobs:**
   ```toml
   [transcode]
   concurrent_jobs = 1
   ```

2. **Lower thread count:**
   ```toml
   [transcode]
   threads = 4  # Use fewer threads
   ```

3. **Use faster presets:**
   ```toml
   [hardware]
   cpu_preset = "fast"
   ```

### Out of Memory Errors

**Solutions:**

1. **Reduce concurrent jobs:**
   ```toml
   [transcode]
   concurrent_jobs = 1
   ```

2. **Close other applications** during encoding

3. **Use H.264** instead of AV1/HEVC:
   ```toml
   [transcode]
   output_codec = "h264"
   ```

### Slow Encoding Speed

**Expected encoding speeds** (1080p content):

| Codec | Preset | Typical Speed |
|-------|--------|---------------|
| **AV1** | Medium | 0.5-1.5x realtime |
| **HEVC** | Medium | 1-3x realtime |
| **H.264** | Medium | 3-8x realtime |

**Solutions for slow speeds:**

1. **Use faster presets:**
   ```toml
   [hardware]
   cpu_preset = "fast"
   ```

2. **Switch codecs:**
   ```toml
   [transcode]
   output_codec = "h264"  # Fastest
   ```

3. **Verify CPU boost** is working:
   ```bash
   # Linux
   cat /proc/cpuinfo | grep MHz
   
   # macOS
   sysctl -a | grep freq
   ```

### Quality Issues

**Solutions:**

1. **Lower CRF values** (better quality):
   ```toml
   [transcode.encoder_args]
   extra_args = ["-crf", "24"]  # Lower = better quality
   ```

2. **Use slower presets:**
   ```toml
   [hardware]
   cpu_preset = "slow"
   ```

3. **Enable quality features:**
   ```toml
   # x265 example
   [transcode.encoder_args]
   extra_args = ["-x265-params", "aq-mode=3:aq-strength=1.0"]
   ```

## Best Practices

### When to Use CPU Encoding

**Ideal scenarios:**
- No compatible GPU available
- Maximum quality requirements
- Small batch processing
- Development/testing

**Consider GPU instead when:**
- Processing large libraries
- Speed is priority
- Running 24/7 operations
- High resolution content (4K+)

### Quality vs. Speed Trade-offs

**Maximum quality** (archival):
```toml
[transcode]
output_codec = "av1"
quality_profile = "quality"
concurrent_jobs = 1

[hardware]
cpu_preset = "slow"
```

**Balanced performance** (recommended):
```toml
[transcode]
output_codec = "hevc"
quality_profile = "balanced"
concurrent_jobs = 2  # Adjust for your CPU

[hardware]
cpu_preset = "medium"
```

**Speed priority** (quick results):
```toml
[transcode]
output_codec = "h264"
quality_profile = "speed"
concurrent_jobs = 4  # More jobs, fewer threads each

[hardware]
cpu_preset = "fast"
```

### System Optimization

**Linux optimizations:**
```bash
# Set CPU governor to performance
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Disable CPU frequency scaling
echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo
```

**Windows optimizations:**
- Set power plan to "High Performance"
- Disable CPU parking in registry
- Close unnecessary background apps

**macOS optimizations:**
- Use Activity Monitor to verify CPU usage
- Close other intensive applications
- Consider thermal throttling on laptops