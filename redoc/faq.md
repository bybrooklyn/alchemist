# Frequently Asked Questions

Comprehensive FAQ covering everything you need to know about Alchemist.

# Frequently Asked Questions

This comprehensive FAQ answers the most common questions about Alchemist. Questions are organized by topic for easy navigation.

## General Questions

### What exactly does Alchemist do?

Alchemist is a **smart video transcoding pipeline** that automatically converts your media library to more efficient formats. Think of it as a "garbage compactor" for video files that:

- Scans your media collection automatically
- Analyzes each file to determine if transcoding would help
- Uses hardware acceleration (GPU) when available
- Only transcodes files that will actually benefit
- Preserves your originals until you're satisfied with results
- Provides a web dashboard to monitor everything

### Is Alchemist free?

**Yes, completely free!** Alchemist is open-source software released under the GPLv3 license. This means:

- No cost to download, install, or use
- Source code is publicly available
- No premium features or subscriptions
- No telemetry or tracking (unless you explicitly enable it)
- Community-driven development

### How much storage will I save?

Typical savings depend on your source content:

| Content Type | Typical Savings | Example |
|-------------|----------------|---------|
| **Old TV shows** (DVD/early digital) | 50-80% | 2GB → 600MB |
| **Blu-ray rips** (H.264) | 30-60% | 25GB → 12GB |
| **Modern streaming** (already efficient) | 0-20% | Often skipped |
| **4K content** | 40-70% | 60GB → 25GB |

**Average across mixed libraries: 40-60% space savings**

### Will Alchemist ruin my video quality?

**No.** Alchemist is designed with quality protection:

- **Intelligent analysis** checks if transcoding will help before starting
- **Quality thresholds** prevent processing files that are already efficient
- **VMAF scoring** (optional) verifies output quality mathematically
- **Conservative defaults** prioritize quality over maximum compression
- **Originals preserved** until you manually approve deletion

If Alchemist thinks a transcode would hurt quality, it skips the file and tells you why.

### Does it work on Windows, Mac, and Linux?

**Yes, all three.** Alchemist works on:

| Platform | Status | Notes |
|----------|--------|-------|
| **Linux x86_64/ARM64** | ✅ Fully supported | Best performance |
| **Windows x86_64** | ✅ Fully supported | Good GPU support |
| **macOS Intel/Apple Silicon** | ✅ Fully supported | VideoToolbox acceleration |
| **Docker** | ✅ Recommended | Works anywhere Docker runs |

## Hardware & Performance

### Do I need a powerful graphics card?

**No, but it helps a lot.** Alchemist works in any configuration:

- **With GPU**: 20-60 minutes per movie, low power usage
- **CPU only**: 2-8 hours per movie, higher power usage
- **Automatic fallback**: Uses GPU when available, CPU when not

**Supported GPUs:**
- NVIDIA (GTX 10-series and newer)
- Intel integrated graphics (6th gen and newer) 
- AMD Radeon (RX 400-series and newer)
- Apple Silicon (M1/M2/M3)

### What's the difference between GPU and CPU encoding?

| Aspect | GPU Encoding | CPU Encoding |
|--------|--------------|--------------|
| **Speed** | 5-20x faster | Baseline |
| **Quality** | Very good | Excellent |
| **Power usage** | Lower | Higher |
| **Compatibility** | Requires supported GPU | Works everywhere |
| **Cost** | GPU hardware needed | Uses existing CPU |

**Bottom line**: GPU encoding is much faster and more efficient, but CPU encoding produces slightly better quality.

### Can I limit when Alchemist runs?

**Yes!** Multiple ways to control when processing happens:

1. **Engine modes**:
   - **Background**: Minimal resource usage
   - **Balanced**: Moderate performance (default)
   - **Throughput**: Maximum performance

2. **Scheduling**:
   ```toml
   [schedule]
   [[schedule.windows]]
   start_time = "22:00"  # 10 PM
   end_time = "06:00"    # 6 AM
   days_of_week = [1, 2, 3, 4, 5]  # Weekdays only
   ```

3. **Manual control**: Pause/resume anytime from the dashboard

### How many files can it process simultaneously?

Depends on your hardware:

| System Type | Recommended Concurrent Jobs |
|-------------|----------------------------|
| **Basic CPU** (4 cores) | 1 |
| **Good CPU** (8+ cores) | 2 |
| **GPU + good CPU** | 2-3 |
| **High-end workstation** | 4+ |

The system auto-adjusts based on available resources, but you can override:

```toml
[transcode]
concurrent_jobs = 2  # Manual override
```

## Quality & Codecs

### What's the difference between AV1, HEVC, and H.264?

| Codec | Compression | Compatibility | Encoding Speed | Best For |
|-------|-------------|---------------|----------------|----------|
| **AV1** | Excellent (30% better than HEVC) | Newer devices | Slower | Future-proofing, archival |
| **HEVC/H.265** | Very good (50% better than H.264) | Most modern devices | Medium | General use, good balance |
| **H.264** | Good (baseline) | Universal | Fastest | Compatibility, quick results |

**Recommendation**: Start with HEVC for best balance of quality, compatibility, and encoding speed.

### Should I enable VMAF quality checking?

**VMAF** is Netflix's quality measurement tool. Enable if:

✅ **Yes, if you:**
- Have critical content you can't re-encode
- Want mathematical quality verification
- Don't mind 2-3x slower encoding
- Are a quality enthusiast

❌ **No, if you:**
- Want fast processing
- Trust Alchemist's quality settings
- Have large libraries to process
- Use reasonable quality settings already

### What happens to HDR content?

Alchemist can handle HDR content two ways:

1. **Preserve HDR** (default):
   ```toml
   [transcode]
   hdr_mode = "preserve"
   ```
   - Keeps HDR metadata intact
   - Requires HDR-capable display for viewing
   - Smaller file size impact

2. **Tonemap to SDR**:
   ```toml
   [transcode]
   hdr_mode = "tonemap"
   tonemap_algorithm = "hable"  # Recommended
   ```
   - Converts to standard dynamic range
   - Works on any display
   - Slight quality loss in bright scenes

### Can I customize quality settings per library?

**Yes!** Use different profiles for different content:

```toml
# Movies: Maximum quality
[profiles.movies]
quality_profile = "quality"
output_codec = "av1"
min_file_size_mb = 500  # Only large files

# TV Shows: Faster processing
[profiles.tv_shows] 
quality_profile = "speed"
output_codec = "hevc"
min_file_size_mb = 100

# Home videos: Preserve originals
[profiles.home_videos]
delete_source = false
output_codec = "h264"
```

## File Management

### What happens to my original files?

**By default, originals are kept safe.** Alchemist:

1. Creates new file with `-alchemist` suffix
2. Verifies the new file works correctly
3. Keeps both files until you decide

**Options for originals:**
- **Keep both** (default, safest)
- **Manual review** then delete originals
- **Auto-delete** after successful transcode (risky)

```toml
[files]
delete_source = false  # Keep originals (recommended)
output_suffix = "-alchemist"
replace_strategy = "keep"  # Don't overwrite existing files
```

### Can I organize output files differently?

**Yes!** Several organization options:

1. **Same location with suffix** (default):
   ```
   /media/Movie.mkv
   /media/Movie-alchemist.mkv
   ```

2. **Separate output directory**:
   ```toml
   [files]
   output_root = "/media/transcoded"
   ```
   Result:
   ```
   /media/movies/Movie.mkv (original)
   /media/transcoded/movies/Movie.mkv (transcoded)
   ```

3. **Custom file extensions**:
   ```toml
   [files]
   output_extension = "mp4"  # Change container format
   ```

### How do I handle different languages and audio tracks?

**Stream rules** let you customize audio handling:

```toml
[transcode.stream_rules]
# Remove commentary tracks
strip_audio_by_title = ["commentary", "director", "behind"]

# Keep only English and Japanese audio
keep_audio_languages = ["eng", "jpn"]

# Or keep only the default audio track
keep_only_default_audio = true
```

**Audio encoding options**:
```toml
[transcode]
audio_mode = "copy"        # Keep original (recommended)
# audio_mode = "aac"       # Transcode to AAC
# audio_mode = "aac_stereo" # Downmix to stereo AAC
```

## Setup & Configuration

### Docker vs. binary installation - which should I choose?

| Method | Pros | Cons | Best For |
|--------|------|------|----------|
| **Docker** | ✅ Easy setup<br>✅ All dependencies included<br>✅ Consistent across systems<br>✅ Easy updates | ❌ Slightly more complex config<br>❌ Docker overhead | Most users, especially beginners |
| **Binary** | ✅ Direct system access<br>✅ Lower overhead<br>✅ No Docker complexity | ❌ Manual dependency management<br>❌ Platform-specific issues | Advanced users, specialized setups |

**Recommendation**: Use Docker unless you have specific needs for binary installation.

### How do I update Alchemist?

**Docker update**:
```bash
# Pull latest image
docker pull ghcr.io/bybrooklyn/alchemist:latest

# Restart container
docker compose down && docker compose up -d
```

**Binary update**:
1. Download new binary from GitHub releases
2. Stop current Alchemist instance
3. Replace binary file
4. Restart Alchemist

**Database migrations** are automatic - your settings and history are preserved.

### Can I run multiple Alchemist instances?

**Generally no** - Alchemist is designed as a single-instance application. However:

✅ **Supported scenarios**:
- Different libraries on different machines
- Test instance with separate config/database

❌ **Not supported**:
- Multiple instances accessing the same library
- Multiple instances sharing a database
- Load balancing across instances

For high-performance setups, use:
- Higher concurrent job count
- Faster hardware
- Multiple GPUs in single instance (future feature)

## Troubleshooting & Support

### Why aren't my files being processed?

**Common reasons files get skipped**:

1. **Too small**: Below `min_file_size_mb` threshold
2. **Already efficient**: Below `size_reduction_threshold`
3. **Good quality**: Above `min_bpp_threshold`
4. **Wrong format**: Not a supported video file
5. **File errors**: Corrupted or unreadable

Check the **Library Doctor** for detailed analysis of why files were skipped.

### How do I know if hardware acceleration is working?

**Check the Dashboard**:
- Hardware status shows detected GPU
- Job details show encoder being used

**Check system monitors**:
```bash
# NVIDIA
nvidia-smi

# Intel  
intel_gpu_top

# AMD
radeontop

# General
htop  # Look for low CPU usage during encoding
```

**Look for logs**:
```
[INFO] Using NVENC for encoding
[INFO] Hardware encoder initialized: hevc_nvenc
```

### What are "Library Doctor" issues?

**Library Doctor** scans your media for problems:

- **Corrupted files**: Won't play properly
- **Encoding errors**: Video/audio sync issues  
- **Missing data**: Incomplete downloads
- **Format issues**: Unusual codecs or containers

It's a **diagnostic tool** - not all issues need fixing, but you should be aware of them.

### Performance is slower than expected - what to check?

**Diagnosis checklist**:

1. **Verify hardware acceleration**:
   - Check dashboard shows GPU detected
   - Monitor GPU usage during encoding

2. **Check system resources**:
   - CPU usage (should be low with GPU)
   - RAM availability
   - Disk speed (especially important for 4K)

3. **Optimize settings**:
   ```toml
   [transcode]
   quality_profile = "speed"  # Faster encoding
   concurrent_jobs = 1        # Reduce if system struggles
   ```

4. **Check thermal throttling**:
   - Monitor CPU/GPU temperatures
   - Ensure adequate cooling

### Getting help with issues

**Before asking for help**:

1. **Check this FAQ** and the [Troubleshooting Guide](troubleshooting.md)
2. **Search existing issues** on GitHub
3. **Enable debug logging**: `RUST_LOG=debug`

**When reporting issues, include**:
- System information (OS, hardware)
- Alchemist version
- Configuration file (remove sensitive data)
- Relevant log excerpts
- Steps to reproduce the problem

**Where to get help**:
- **GitHub Issues**: [github.com/bybrooklyn/alchemist/issues](https://github.com/bybrooklyn/alchemist/issues)
- **Documentation**: This site
- **Community**: GitHub Discussions

## Advanced Usage

### Can I customize the FFmpeg commands?

**Limited customization** is available through encoder args:

```toml
[transcode.encoder_args]
# Example: Custom quality settings
extra_args = [
  "-crf", "22",           # Custom quality level
  "-preset", "slower",    # Custom speed preset
  "-tune", "film"         # Optimize for film content
]
```

**Note**: Full FFmpeg customization isn't supported to maintain reliability and quality consistency.

### How do I migrate from other transcoding tools?

**From Tdarr**:
- Export your Tdarr library database
- Point Alchemist at the same directories
- Let Alchemist re-scan and analyze files
- Alchemist will skip already-optimized files

**From HandBrake batch scripts**:
- Point Alchemist at your source directories
- Configure similar quality settings
- Alchemist automates the batch process

**From other tools**:
- Most tools can coexist with Alchemist
- Use different output suffixes to avoid conflicts
- Alchemist focuses on automation vs. manual control

### Can I use Alchemist in a production environment?

**Alchemist is designed for home users** but can work in professional contexts:

✅ **Good for**:
- Personal media servers
- Small office setups
- Content creators' personal libraries
- Automated archival workflows

⚠️ **Consider limitations**:
- Single-instance design
- Limited customization
- Home-focused feature set
- Community support only

For enterprise needs, consider commercial solutions or custom development.

### Integration with media servers (Plex, Jellyfin, etc.)

**Alchemist works alongside media servers**:

1. **Point Alchemist** at your media directories
2. **Configure output** to same location or separate folder
3. **Media servers** automatically detect transcoded files
4. **Use scheduling** to avoid conflicts during peak usage

**Best practices**:
- Run during off-peak hours
- Test with small batches first
- Monitor media server performance
- Keep originals until you verify everything works

**Common workflow**:
```
Media files → Alchemist → Optimized files → Media server → Streaming
```

This setup gives you both the convenience of automated transcoding and the features of your preferred media server.
