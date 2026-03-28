# Configuration Reference

Complete reference for all Alchemist configuration options with examples.

# Configuration Reference

This page provides a comprehensive reference for all Alchemist configuration options. The configuration file is written in TOML format and contains settings that control every aspect of how Alchemist processes your media.

## Configuration File Location

The configuration file is automatically created during the setup wizard. Default locations:

- **Linux/macOS**: `~/.config/alchemist/config.toml`
- **Windows**: `%APPDATA%\alchemist\config.toml` 
- **Docker**: Bind mount to `/app/config/config.toml`

You can override the location with the `ALCHEMIST_CONFIG_PATH` environment variable.

## Sample Configuration

Here's a complete example configuration file:

```toml
[appearance]
active_theme_id = "dark"

[transcode]
size_reduction_threshold = 0.3
min_bpp_threshold = 0.1  
min_file_size_mb = 100
concurrent_jobs = 2
threads = 0
quality_profile = "balanced"
output_codec = "av1"
allow_fallback = true
hdr_mode = "preserve"
tonemap_algorithm = "hable"
tonemap_peak = 100.0
tonemap_desat = 0.2
subtitle_mode = "copy"
vmaf_min_score = 93.0

[transcode.stream_rules]
strip_audio_by_title = ["commentary", "director"]
keep_audio_languages = ["eng"]
keep_only_default_audio = false

[hardware]
preferred_vendor = "nvidia"
device_path = "/dev/dri/renderD128"
allow_cpu_fallback = true
cpu_preset = "medium"
allow_cpu_encoding = true

[scanner]
directories = ["/media/movies", "/media/tv"]
watch_enabled = true

[[scanner.extra_watch_dirs]]
path = "/media/incoming"
is_recursive = true

[notifications]
enabled = true
notify_on_complete = true
notify_on_failure = true

[[notifications.targets]]
name = "discord"
target_type = "discord"
endpoint_url = "https://discord.com/api/webhooks/..."
enabled = true
events = ["job_complete", "job_failed"]

[files]
delete_source = false
output_extension = "mkv"
output_suffix = "-alchemist"
replace_strategy = "keep"
output_root = "/media/transcoded"

[schedule]
[[schedule.windows]]
start_time = "22:00"
end_time = "06:00"  
days_of_week = [1, 2, 3, 4, 5]
enabled = true

[quality]
enable_vmaf = true
min_vmaf_score = 90.0
revert_on_low_quality = true

[system]
monitoring_poll_interval = 2.0
enable_telemetry = false
log_retention_days = 30
engine_mode = "balanced"
https_only = false
```

## Section Reference

### Appearance (`appearance`)

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `active_theme_id` | string | `null` | Theme ID for the web interface ("light", "dark", or custom) |

### Transcoding (`transcode`)

Core settings that control the video encoding process.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `size_reduction_threshold` | float | `0.3` | Minimum expected size reduction (0.3 = 30%) to proceed with transcoding |
| `min_bpp_threshold` | float | `0.1` | Minimum bits-per-pixel to consider a file worth transcoding |
| `min_file_size_mb` | integer | `50` | Skip files smaller than this (in MB) |
| `concurrent_jobs` | integer | `1` | Number of simultaneous transcoding jobs |
| `threads` | integer | `0` | CPU threads per job (0 = automatic) |
| `quality_profile` | string | `"balanced"` | Speed vs quality tradeoff: `"quality"`, `"balanced"`, `"speed"` |
| `output_codec` | string | `"av1"` | Target codec: `"av1"`, `"hevc"`, `"h264"` |
| `allow_fallback` | boolean | `true` | Fall back to CPU if hardware encoding fails |
| `hdr_mode` | string | `"preserve"` | HDR handling: `"preserve"` or `"tonemap"` |
| `tonemap_algorithm` | string | `"hable"` | Tonemap method: `"hable"`, `"mobius"`, `"reinhard"`, `"clip"` |
| `tonemap_peak` | float | `100.0` | Target peak luminance for tonemapping (nits) |
| `tonemap_desat` | float | `0.2` | Desaturation factor during tonemapping |
| `subtitle_mode` | string | `"copy"` | Subtitle handling: `"copy"`, `"burn"`, `"extract"`, `"none"` |
| `vmaf_min_score` | float | `null` | Minimum VMAF score to accept transcode (optional) |

#### Quality Profiles

Each profile adjusts encoding parameters for different priorities:

- **Quality**: Slower encoding, best compression (CRF 24, preset slow)
- **Balanced**: Good compromise (CRF 28, preset medium) 
- **Speed**: Faster encoding, larger files (CRF 32, preset fast)

#### Stream Rules (`transcode.stream_rules`)

Audio track filtering rules applied before transcoding:

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `strip_audio_by_title` | array | `[]` | Remove tracks containing these strings (case-insensitive) |
| `keep_audio_languages` | array | `[]` | Keep only tracks with these ISO 639-2 language codes |
| `keep_only_default_audio` | boolean | `false` | Keep only the default audio track |

### Hardware (`hardware`)

Graphics card and CPU encoding settings.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `preferred_vendor` | string | `null` | Preferred encoder: `"nvidia"`, `"intel"`, `"amd"`, `"apple"`, `"cpu"` |
| `device_path` | string | `null` | Specific GPU device path (e.g., `/dev/dri/renderD128`) |
| `allow_cpu_fallback` | boolean | `true` | Use CPU encoding if GPU unavailable |
| `cpu_preset` | string | `"medium"` | CPU preset: `"slow"`, `"medium"`, `"fast"`, `"faster"` |
| `allow_cpu_encoding` | boolean | `true` | Enable CPU encoding entirely |

### Scanner (`scanner`) 

File discovery and monitoring settings.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `directories` | array | `[]` | Base directories to scan for media files |
| `watch_enabled` | boolean | `false` | Enable real-time file monitoring |
| `extra_watch_dirs` | array | `[]` | Additional directories with custom settings |

#### Extra Watch Directories

Each entry in `extra_watch_dirs` supports:

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `path` | string | Required | Directory path to monitor |
| `is_recursive` | boolean | `true` | Include subdirectories |

### Notifications (`notifications`)

Alert settings for job completion and failures.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable notification system |
| `notify_on_complete` | boolean | `false` | Send alerts when jobs complete successfully |
| `notify_on_failure` | boolean | `false` | Send alerts when jobs fail |
| `targets` | array | `[]` | Notification endpoints (see below) |

#### Notification Targets

Each target in the `targets` array supports:

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `name` | string | Required | Friendly name for this target |
| `target_type` | string | Required | Type: `"discord"`, `"gotify"`, `"webhook"` |
| `endpoint_url` | string | Required | Full URL for the notification service |
| `auth_token` | string | `null` | Authentication token if required |
| `events` | array | `[]` | Events to send: `"job_complete"`, `"job_failed"` |
| `enabled` | boolean | `true` | Whether this target is active |

### Files (`files`)

Output file naming and handling preferences.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `delete_source` | boolean | `false` | Delete original files after successful transcode |
| `output_extension` | string | `"mkv"` | Container format for output files |
| `output_suffix` | string | `"-alchemist"` | Suffix added to transcoded filenames |
| `replace_strategy` | string | `"keep"` | How to handle existing output files: `"keep"`, `"overwrite"` |
| `output_root` | string | `null` | Alternative output directory (preserves folder structure) |

### Schedule (`schedule`)

Time-based restrictions for when transcoding can occur.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `windows` | array | `[]` | Time windows when transcoding is allowed |

#### Schedule Windows  

Each window supports:

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `start_time` | string | Required | Start time in 24-hour format (HH:MM) |
| `end_time` | string | Required | End time in 24-hour format (HH:MM) |
| `days_of_week` | array | `[]` | Days 0-6 (Sunday=0) when this window applies |
| `enabled` | boolean | `true` | Whether this window is active |

### Quality (`quality`)

Advanced quality verification settings.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `enable_vmaf` | boolean | `false` | Enable VMAF quality scoring (slow) |
| `min_vmaf_score` | float | `90.0` | Minimum VMAF score to accept transcode |
| `revert_on_low_quality` | boolean | `true` | Delete output and keep original if VMAF score too low |

### System (`system`)

Application-level settings and monitoring.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `monitoring_poll_interval` | float | `2.0` | Seconds between system status updates |
| `enable_telemetry` | boolean | `false` | Send anonymous usage statistics |
| `log_retention_days` | integer | `30` | Days to keep log files (null = forever) |
| `engine_mode` | string | `"balanced"` | Processing mode: `"background"`, `"balanced"`, `"throughput"` |
| `https_only` | boolean | `false` | Enforce HTTPS (only enable behind reverse proxy) |

#### Engine Modes

- **Background**: Minimal resource usage (1 job max)
- **Balanced**: Reasonable performance (½ CPU cores, max 4 jobs)
- **Throughput**: Maximum performance (½ CPU cores, no job limit)

## Environment Variables

Alchemist respects these environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `ALCHEMIST_CONFIG_PATH` | Path to config file | `~/.config/alchemist/config.toml` |
| `ALCHEMIST_DB_PATH` | Path to database file | `~/.config/alchemist/alchemist.db` |
| `ALCHEMIST_CONFIG_MUTABLE` | Allow runtime config changes | `true` |
| `RUST_LOG` | Logging level | `info` |
| `PORT` | Web server port | `3000` |
| `HOST` | Web server bind address | `0.0.0.0` |

## Configuration Validation

Alchemist validates your configuration on startup and will show specific error messages for:
- Invalid values or types
- Missing required fields  
- Conflicting settings
- Unreachable file paths

Check the logs if Alchemist fails to start after configuration changes.
