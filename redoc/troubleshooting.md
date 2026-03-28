# Troubleshooting Guide

Comprehensive guide to diagnosing and solving common Alchemist issues.

# Troubleshooting Guide

This comprehensive guide helps you diagnose and resolve common issues with Alchemist. Issues are organized by category with step-by-step solutions.

## General Diagnostics

### Log Locations

**Default log locations:**
- **Linux/macOS**: `~/.config/alchemist/logs/`
- **Windows**: `%APPDATA%\alchemist\logs\`  
- **Docker**: `/app/data/logs/` (if mounted)

**View recent logs:**
```bash
# Real-time log monitoring
tail -f ~/.config/alchemist/logs/alchemist.log

# Last 100 lines
tail -100 ~/.config/alchemist/logs/alchemist.log

# Search for errors
grep -i error ~/.config/alchemist/logs/alchemist.log
```

### Debug Mode

Enable detailed logging:
```bash
# Environment variable
RUST_LOG=debug ./alchemist

# Or in Docker
docker run -e RUST_LOG=debug ghcr.io/bybrooklyn/alchemist:latest
```

### System Health Check

Verify your setup:
```bash
# Check FFmpeg
ffmpeg -version

# Test hardware encoders
ffmpeg -encoders | grep -E "nvenc|qsv|vaapi|amf|videotoolbox"

# Check disk space
df -h

# Monitor system resources
htop  # or top on basic systems
```

## Authentication & Access Issues

### "Failed to load settings" / 401 Unauthorized

**Symptoms:**
- Can't access web interface
- "Authentication failed" messages
- Redirected to login repeatedly

**Solutions:**

1. **Clear browser storage:**
   ```javascript
   // Open browser dev tools (F12), then Console tab
   localStorage.clear();
   sessionStorage.clear();
   location.reload();
   ```

2. **Reset admin password:**
   ```bash
   # Stop Alchemist, then reset database auth
   sqlite3 ~/.config/alchemist/alchemist.db "DELETE FROM user_sessions;"
   sqlite3 ~/.config/alchemist/alchemist.db "UPDATE users SET password_hash = '' WHERE username = 'admin';"
   ```

3. **Check configuration:**
   ```toml
   [system]
   https_only = false  # Ensure this is false unless using HTTPS
   ```

### Can't Access Web Interface

**Symptoms:**
- Browser shows "connection refused"
- Timeout errors
- Blank page loads

**Solutions:**

1. **Verify Alchemist is running:**
   ```bash
   # Check process
   ps aux | grep alchemist
   
   # Check port binding
   netstat -tlnp | grep 3000
   ```

2. **Check firewall settings:**
   ```bash
   # Linux (ufw)
   sudo ufw allow 3000
   
   # Linux (firewall-cmd)
   sudo firewall-cmd --add-port=3000/tcp --permanent
   sudo firewall-cmd --reload
   ```

3. **Try different browsers/incognito mode**

4. **Check Docker port mapping:**
   ```yaml
   # Ensure correct port mapping in docker-compose.yml
   ports:
     - "3000:3000"  # Host:Container
   ```

## Hardware Detection Issues

### "No hardware encoder detected"

**Symptoms:**
- Only CPU encoding available
- Hardware shows as "Not detected"
- Slow transcoding speeds

**Diagnosis steps:**

1. **Check hardware detection:**
   ```bash
   # NVIDIA
   nvidia-smi
   
   # Intel
   ls -la /dev/dri/
   
   # AMD 
   lspci | grep -i amd
   
   # Apple (macOS)
   system_profiler SPHardwareDataType
   ```

2. **Verify FFmpeg support:**
   ```bash
   ffmpeg -encoders | grep -E "nvenc|qsv|vaapi|amf|videotoolbox"
   ```

**Solutions by vendor:**

#### NVIDIA Issues
```bash
# Install NVIDIA Container Toolkit (Docker)
# Ubuntu/Debian:
curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
distribution=$(. /etc/os-release;echo $ID$VERSION_ID)
curl -s -L https://nvidia.github.io/libnvidia-container/$distribution/libnvidia-container.list | \
  sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
  sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list
sudo apt-get update && sudo apt-get install -y nvidia-container-toolkit
sudo systemctl restart docker

# Test GPU access in container
docker run --rm --gpus all nvidia/cuda:12.0-base nvidia-smi
```

#### Intel Issues
```bash
# Ensure iGPU is enabled in BIOS
# Add user to video/render groups
sudo usermod -a -G video,render $USER

# Install Intel media drivers (Ubuntu/Debian)
sudo apt install intel-media-va-driver libva-utils

# Test VAAPI
vainfo --display drm --device /dev/dri/renderD128
```

#### AMD Issues
```bash
# Install Mesa drivers (Ubuntu/Debian)
sudo apt install mesa-va-drivers libva-utils

# Add user to video/render groups
sudo usermod -a -G video,render $USER

# Test VAAPI
vainfo --display drm --device /dev/dri/renderD128
```

### Hardware Detected But Encoding Fails

**Symptoms:**
- Hardware shows as detected
- Encoding jobs fail with GPU errors
- Falls back to CPU

**Solutions:**

1. **Check GPU memory:**
   ```bash
   # NVIDIA
   nvidia-smi
   
   # Intel/AMD - check system memory if integrated
   free -h
   ```

2. **Reduce concurrent jobs:**
   ```toml
   [transcode]
   concurrent_jobs = 1  # Start with 1
   ```

3. **Update drivers:**
   - NVIDIA: Download from nvidia.com
   - Intel: Update through Windows Update or Intel Driver Assistant
   - AMD: Download from amd.com/support

4. **Check for conflicting processes:**
   ```bash
   # See what's using the GPU
   nvidia-smi  # NVIDIA
   intel_gpu_top  # Intel
   ```

## Processing Issues

### Jobs Stuck in "Queued" State

**Symptoms:**
- Jobs never start processing
- Queue doesn't advance
- Dashboard shows "paused" or "idle"

**Solutions:**

1. **Check engine status:**
   - Navigate to Dashboard
   - Look for "Paused" indicator
   - Click "Resume" if available

2. **Check system resources:**
   ```bash
   # CPU usage
   top
   
   # Memory usage
   free -h
   
   # Disk space
   df -h
   ```

3. **Restart the processor:**
   ```bash
   # Binary installation
   pkill alchemist
   ./alchemist

   # Docker
   docker restart alchemist
   ```

4. **Check scheduling windows:**
   ```toml
   # Ensure schedule allows current time
   [schedule]
   [[schedule.windows]]
   start_time = "00:00"  # 24/7 operation
   end_time = "23:59"
   enabled = true
   ```

### Jobs Fail Immediately

**Symptoms:**
- Jobs start but fail within seconds
- "Encoding failed" messages in logs
- No output files created

**Diagnosis:**

1. **Check specific error in logs:**
   ```bash
   grep -A5 -B5 "failed" ~/.config/alchemist/logs/alchemist.log
   ```

2. **Test FFmpeg command manually:**
   ```bash
   # Extract the failed command from logs and test
   ffmpeg -i input.mkv -c:v libx264 -crf 23 test_output.mkv
   ```

**Common solutions:**

1. **File permission issues:**
   ```bash
   # Check file permissions
   ls -la /path/to/media/
   
   # Fix permissions if needed
   chmod 644 /path/to/media/*
   ```

2. **Corrupt source files:**
   ```bash
   # Test source file
   ffmpeg -v error -i input.mkv -f null -
   ```

3. **Insufficient disk space:**
   ```bash
   # Check available space
   df -h
   
   # Clean up if needed
   du -sh ~/.config/alchemist/logs/* | sort -h
   ```

### Poor Quality Output

**Symptoms:**
- Encoded files look worse than originals
- Artifacts or blocking visible
- Low VMAF scores

**Solutions:**

1. **Adjust quality settings:**
   ```toml
   [transcode]
   quality_profile = "quality"  # Use highest quality
   
   # Or manually adjust CRF
   [transcode.encoder_args]
   extra_args = ["-crf", "20"]  # Lower = better quality
   ```

2. **Check source file quality:**
   ```bash
   # Analyze source with ffprobe
   ffprobe -v quiet -show_format -show_streams input.mkv
   ```

3. **Enable quality verification:**
   ```toml
   [quality]
   enable_vmaf = true
   min_vmaf_score = 92.0  # Reject low quality transcodes
   revert_on_low_quality = true
   ```

4. **Use appropriate codec:**
   ```toml
   # For maximum quality
   [transcode]
   output_codec = "hevc"  # Better than H.264
   quality_profile = "quality"
   ```

## Performance Issues

### High CPU Usage During Encoding

**Symptoms:**
- System becomes unresponsive
- High CPU temperatures
- Fan noise increases

**Solutions:**

1. **Verify hardware acceleration:**
   ```bash
   # Check if GPU is being used
   nvidia-smi  # Should show ffmpeg processes
   ```

2. **Reduce CPU load:**
   ```toml
   [transcode]
   concurrent_jobs = 1
   threads = 4  # Limit CPU threads
   
   [system]
   engine_mode = "background"  # Minimal resource usage
   ```

3. **Enable hardware acceleration:**
   ```toml
   [hardware]
   preferred_vendor = "nvidia"  # or intel/amd/apple
   allow_cpu_fallback = false  # Force hardware encoding
   ```

### Slow Encoding Speeds

**Expected speeds** for reference:
- **GPU encoding**: 1-5x realtime
- **CPU encoding**: 0.1-2x realtime

**Solutions:**

1. **Check hardware utilization:**
   ```bash
   # GPU usage
   nvidia-smi -l 1  # NVIDIA
   intel_gpu_top    # Intel
   
   # CPU usage
   htop
   ```

2. **Optimize settings for speed:**
   ```toml
   [transcode]
   quality_profile = "speed"
   output_codec = "h264"  # Fastest encoding
   
   [hardware]
   cpu_preset = "fast"  # If using CPU fallback
   ```

3. **Check thermal throttling:**
   ```bash
   # Linux
   sensors
   
   # macOS  
   sudo powermetrics --samplers smc -n 1 | grep -i temp
   ```

### High Memory Usage

**Symptoms:**
- System uses excessive RAM
- Out of memory errors
- System becomes unstable

**Solutions:**

1. **Reduce memory usage:**
   ```toml
   [transcode]
   concurrent_jobs = 1  # Reduce parallel processing
   threads = 4          # Lower thread count
   ```

2. **Check for memory leaks:**
   ```bash
   # Monitor Alchemist memory usage
   ps aux | grep alchemist
   
   # Monitor over time
   while true; do ps -p $(pgrep alchemist) -o pid,ppid,cmd,%mem,%cpu; sleep 30; done
   ```

3. **Restart periodically:**
   ```bash
   # Set up log rotation and periodic restart
   # Add to crontab for daily restart
   0 6 * * * docker restart alchemist
   ```

## Database Issues

### Database Locked Errors

**Symptoms:**
- "Database is locked" in logs
- Web interface becomes unresponsive
- Jobs don't update status

**Solutions:**

1. **Stop all Alchemist processes:**
   ```bash
   # Kill all instances
   pkill -f alchemist
   
   # Or for Docker
   docker stop alchemist
   ```

2. **Check for database corruption:**
   ```bash
   # Test database integrity
   sqlite3 ~/.config/alchemist/alchemist.db "PRAGMA integrity_check;"
   ```

3. **Backup and reset if needed:**
   ```bash
   # Backup current database
   cp ~/.config/alchemist/alchemist.db ~/.config/alchemist/alchemist.db.backup
   
   # If corrupted, reset (loses job history)
   rm ~/.config/alchemist/alchemist.db
   # Alchemist will recreate on next start
   ```

### Migration Errors

**Symptoms:**
- "Migration failed" on startup
- Database version mismatch errors
- Unable to start after update

**Solutions:**

1. **Backup database before fixes:**
   ```bash
   cp ~/.config/alchemist/alchemist.db ~/.config/alchemist/alchemist.db.pre-fix
   ```

2. **Check database version:**
   ```bash
   sqlite3 ~/.config/alchemist/alchemist.db "PRAGMA user_version;"
   ```

3. **Force migration recovery:**
   ```bash
   # Stop Alchemist first
   # Then try manual schema fix (advanced users only)
   sqlite3 ~/.config/alchemist/alchemist.db
   # Run appropriate CREATE TABLE statements from migration files
   ```

## Network & Connectivity

### API Timeout Errors

**Symptoms:**
- Web interface loads slowly
- "Request timeout" errors
- Incomplete data loading

**Solutions:**

1. **Check system load:**
   ```bash
   uptime
   htop
   ```

2. **Increase timeout values:**
   ```toml
   [system]
   monitoring_poll_interval = 5.0  # Slower polling
   ```

3. **Optimize database:**
   ```bash
   # Vacuum database
   sqlite3 ~/.config/alchemist/alchemist.db "VACUUM;"
   
   # Reindex
   sqlite3 ~/.config/alchemist/alchemist.db "REINDEX;"
   ```

### Notification Delivery Issues

**Symptoms:**
- Discord/Gotify notifications not received
- Webhook timeouts
- "Failed to send notification" in logs

**Solutions:**

1. **Test webhook manually:**
   ```bash
   # Test Discord webhook
   curl -X POST -H "Content-Type: application/json" \
     -d '{"content":"Test from Alchemist"}' \
     "YOUR_DISCORD_WEBHOOK_URL"
   ```

2. **Check firewall/network:**
   ```bash
   # Test external connectivity
   curl -I https://discord.com
   
   # Check DNS resolution
   nslookup discord.com
   ```

3. **Verify notification config:**
   ```toml
   [[notifications.targets]]
   name = "discord"
   target_type = "discord"
   endpoint_url = "https://discord.com/api/webhooks/..."
   enabled = true
   events = ["job_complete", "job_failed"]
   ```

## File System Issues

### Permission Denied Errors

**Symptoms:**
- Can't read source files
- Can't write output files
- "Permission denied" in logs

**Solutions:**

1. **Check file ownership:**
   ```bash
   ls -la /path/to/media/
   
   # Fix ownership if needed
   sudo chown -R $USER:$USER /path/to/media/
   ```

2. **Docker user mapping:**
   ```yaml
   services:
     alchemist:
       user: "${UID}:${GID}"  # Match host user
   ```

3. **SELinux/AppArmor issues (Linux):**
   ```bash
   # Check SELinux
   getenforce
   
   # Temporarily disable for testing
   sudo setenforce 0
   
   # Check AppArmor
   sudo aa-status
   ```

### Files Not Found

**Symptoms:**
- "File not found" despite file existing
- Scan doesn't find media files
- Empty libraries

**Solutions:**

1. **Verify paths in config:**
   ```toml
   [scanner]
   directories = ["/correct/path/to/media"]  # Check this path
   ```

2. **Check file extensions:**
   ```bash
   # See what files exist
   find /path/to/media -name "*.mkv" -o -name "*.mp4" -o -name "*.avi" | head -10
   ```

3. **Test file access:**
   ```bash
   # Can Alchemist user access the file?
   stat /path/to/media/movie.mkv
   ```

## Recovery Procedures

### Complete Reset

If all else fails, reset Alchemist to fresh state:

1. **Backup important data:**
   ```bash
   cp ~/.config/alchemist/config.toml ~/alchemist-config-backup.toml
   ```

2. **Stop Alchemist:**
   ```bash
   pkill -f alchemist
   # or
   docker stop alchemist
   ```

3. **Reset database and config:**
   ```bash
   rm -rf ~/.config/alchemist/
   # Alchemist will run setup wizard on next start
   ```

### Partial Recovery

Keep configuration but reset job history:

```bash
# Stop Alchemist
pkill -f alchemist

# Reset only job-related tables
sqlite3 ~/.config/alchemist/alchemist.db << EOF
DELETE FROM jobs;
DELETE FROM job_progress;
DELETE FROM encoding_sessions;
VACUUM;
EOF
```

## Getting Help

When seeking support:

1. **Gather system information:**
   ```bash
   # Create debug info file
   {
     echo "=== System Info ==="
     uname -a
     echo ""
     echo "=== FFmpeg Version ==="
     ffmpeg -version
     echo ""
     echo "=== Hardware Info ==="
     lscpu
     lspci | grep -i vga
     echo ""
     echo "=== Recent Logs ==="
     tail -50 ~/.config/alchemist/logs/alchemist.log
   } > alchemist-debug.txt
   ```

2. **Include configuration** (remove sensitive data):
   ```bash
   # Sanitize config
   sed 's/password.*/password=REDACTED/' ~/.config/alchemist/config.toml > config-sanitized.toml
   ```

3. **Describe the issue:**
   - What were you trying to do?
   - What happened instead?
   - When did it start happening?
   - What changed recently?

4. **Report issues at:**
   - GitHub: [github.com/bybrooklyn/alchemist/issues](https://github.com/bybrooklyn/alchemist/issues)
   - Include debug info and sanitized config
