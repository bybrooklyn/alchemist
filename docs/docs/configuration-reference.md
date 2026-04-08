---
title: Configuration Reference
description: Full configuration reference for Alchemist.
---

Default config file location:

- Linux/macOS: `~/.config/alchemist/config.toml`
- Linux/macOS with XDG: `$XDG_CONFIG_HOME/alchemist/config.toml`
- Windows: `%APPDATA%\Alchemist\config.toml`
- Override: `ALCHEMIST_CONFIG_PATH`

## `[transcode]`

| Field | Type | Default | Description |
|------|------|---------|-------------|
| `size_reduction_threshold` | float | `0.3` | Minimum predicted size reduction required before a transcode is worth doing |
| `min_bpp_threshold` | float | `0.1` | Minimum bits-per-pixel threshold used by the planner to decide whether a file is already efficiently compressed |
| `min_file_size_mb` | int | `50` | Skip files smaller than this size |
| `concurrent_jobs` | int | `1` | Max jobs Alchemist may run at once before engine-mode overrides |
| `threads` | int | `0` | CPU thread count per job; `0` means automatic |
| `quality_profile` | string | `"balanced"` | Quality/speed tradeoff preset |
| `output_codec` | string | `"av1"` | Target codec: `av1`, `hevc`, or `h264` |
| `allow_fallback` | bool | `true` | Allow codec fallback when the requested codec is unavailable |
| `hdr_mode` | string | `"preserve"` | Preserve HDR metadata or tonemap to SDR |
| `tonemap_algorithm` | string | `"hable"` | HDR tonemapping algorithm |
| `tonemap_peak` | float | `100.0` | Tonemap peak luminance target |
| `tonemap_desat` | float | `0.2` | Tonemap desaturation factor |
| `subtitle_mode` | string | `"copy"` | Subtitle handling: `copy`, `burn`, `extract`, or `none` |

## `[transcode.stream_rules]`

| Field | Type | Default | Description |
|------|------|---------|-------------|
| `strip_audio_by_title` | list | `[]` | Remove audio tracks whose title contains any configured case-insensitive substring |
| `keep_audio_languages` | list | `[]` | Keep only audio tracks with matching ISO 639-2 language tags; untagged tracks are kept |
| `keep_only_default_audio` | bool | `false` | Keep only the default audio track after other filters run |

## `[hardware]`

| Field | Type | Default | Description |
|------|------|---------|-------------|
| `preferred_vendor` | string | `auto` | Pin hardware selection to `nvidia`, `intel`, `amd`, `apple`, or `cpu` |
| `device_path` | string | optional | Explicit render node such as `/dev/dri/renderD128` on Linux |
| `allow_cpu_fallback` | bool | `true` | Allow fallback to CPU when no supported GPU path succeeds |
| `cpu_preset` | string | `"medium"` | CPU encoder speed/quality preset |
| `allow_cpu_encoding` | bool | `true` | Allow software encoding at all |

## `[scanner]`

| Field | Type | Default | Description |
|------|------|---------|-------------|
| `directories` | list | `[]` | Library directories to scan |
| `watch_enabled` | bool | `false` | Enable realtime watch behavior for configured directories |
| `extra_watch_dirs` | list | `[]` | Extra watch objects with `path` and `is_recursive` |

## `[notifications]`

| Field | Type | Default | Description |
|------|------|---------|-------------|
| `enabled` | bool | `false` | Master switch for notifications |
| `daily_summary_time_local` | string | `"09:00"` | Global local-time send window for daily summary notifications |
| `targets` | list | `[]` | Notification target objects with `name`, `target_type`, `config_json`, `events`, and `enabled` |

## `[files]`

| Field | Type | Default | Description |
|------|------|---------|-------------|
| `delete_source` | bool | `false` | Delete the original file after a verified successful transcode |
| `output_extension` | string | `"mkv"` | Output file extension |
| `output_suffix` | string | `"-alchemist"` | Suffix added to the output filename |
| `replace_strategy` | string | `"keep"` | Replace behavior for output collisions |
| `output_root` | string | optional | Mirror outputs into another root path instead of writing beside the source |

## `[schedule]`

| Field | Type | Default | Description |
|------|------|---------|-------------|
| `windows` | list | `[]` | Time window objects; each window has `start_time`, `end_time`, and `days_of_week` |

`days_of_week` uses integers `0-6`. The config validator
requires at least one day in every window.

## `[quality]`

| Field | Type | Default | Description |
|------|------|---------|-------------|
| `enable_vmaf` | bool | `false` | Run VMAF scoring after encode |
| `min_vmaf_score` | float | `90.0` | Minimum acceptable VMAF score |
| `revert_on_low_quality` | bool | `true` | Revert the transcode if quality falls below the threshold |

## `[system]`

| Field | Type | Default | Description |
|------|------|---------|-------------|
| `monitoring_poll_interval` | float | `2.0` | Poll interval for system monitoring and dashboard resource refresh |
| `enable_telemetry` | bool | `false` | Opt-in anonymous telemetry switch |
| `log_retention_days` | int | `30` | Log retention period in days |
| `engine_mode` | string | `"balanced"` | Runtime engine mode: `background`, `balanced`, or `throughput` |

## Example

```toml
[transcode]
size_reduction_threshold = 0.3
min_bpp_threshold = 0.1
min_file_size_mb = 50
concurrent_jobs = 1
threads = 0
quality_profile = "balanced"
output_codec = "av1"
allow_fallback = true
hdr_mode = "preserve"
tonemap_algorithm = "hable"
tonemap_peak = 100.0
tonemap_desat = 0.2
subtitle_mode = "copy"

[transcode.stream_rules]
strip_audio_by_title = ["commentary", "description"]
keep_audio_languages = ["eng"]
keep_only_default_audio = false

[hardware]
preferred_vendor = "intel"
allow_cpu_fallback = true
cpu_preset = "medium"
allow_cpu_encoding = true

[scanner]
directories = ["/media/movies", "/media/tv"]
watch_enabled = true

[files]
delete_source = false
output_extension = "mkv"
output_suffix = "-alchemist"
replace_strategy = "keep"

[quality]
enable_vmaf = false
min_vmaf_score = 90.0
revert_on_low_quality = true

[system]
monitoring_poll_interval = 2.0
enable_telemetry = false
log_retention_days = 30
engine_mode = "balanced"
```
