# Alchemist Documentation

> Complete reference for the Alchemist video transcoding automation system

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Installation](#installation)
4. [Configuration](#configuration)
5. [Web Interface](#web-interface)
6. [API Reference](#api-reference)
7. [Database Schema](#database-schema)
8. [Database Migration Policy](#database-migration-policy)
9. [Hardware Acceleration](#hardware-acceleration)
10. [Docker Deployment](#docker-deployment)
11. [Development](#development)
12. [Troubleshooting](#troubleshooting)

---

## Overview

Alchemist is an intelligent video transcoding automation system written in Rust. It automatically analyzes your media library and transcodes files to efficient AV1/HEVC format using hardware acceleration (GPU) or software encoding (CPU fallback).

### Why Alchemist?

Modern video codecs like AV1 and HEVC can reduce file sizes by 30-70% compared to older H.264 content while maintaining visual quality. However, manually transcoding a large media library is tedious and error-prone. Alchemist solves this by:

1. **Intelligent Analysis**: Not every file benefits from re-encoding. Alchemist analyzes bitrate, resolution, and codec efficiency to skip files that are already optimized.

2. **Quality Preservation**: Uses VMAF (Video Multi-Method Assessment Fusion) scoring to ensure transcoded files maintain perceptual quality.

3. **Automatic Rollback**: If a transcode doesn't meet size reduction thresholds, the original file is preserved.

4. **Set-and-Forget**: Configure once, let it run. Watch folders automatically pick up new content.

### Use Cases

| Scenario | Benefit |
|----------|---------|
| **Home Media Server** | Reduce storage costs by shrinking your Plex/Jellyfin library |
| **Archive Optimization** | Convert old DVR recordings to modern efficient formats |
| **Bandwidth Reduction** | Smaller files = faster streaming over network |
| **NAS Storage** | Maximize limited NAS capacity |
| **Content Creators** | Batch convert raw footage to delivery formats |

### Key Features

| Feature | Description |
|---------|-------------|
| **Hardware Acceleration** | NVIDIA NVENC, Intel QSV, AMD VAAPI support |
| **CPU Fallback** | Automatic libsvtav1/x265 encoding when GPU unavailable |
| **Intelligent Analysis** | Only transcodes files that benefit from re-encoding |
| **Web Dashboard** | Real-time monitoring with React/Astro frontend |
| **Single Binary** | All assets embedded for easy deployment |
| **Queue System** | Concurrent job processing with priority support |
| **Statistics** | Detailed metrics, VMAF scores, space savings |
| **Watch Folders** | Auto-enqueue new files in monitored directories |
| **12+ Themes** | Customizable UI with dark mode variants |
| **Auth System** | Secure login with Argon2 password hashing |
| **Real-time Updates** | Server-Sent Events for live progress |

### How It Works

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   SCAN      │-───>│   ANALYZE   │────>│   DECIDE    │────>│   ENCODE    │
│             │     │             │     │             │     │             │
│ Find video  │     │ FFprobe     │     │ Worth it?   │     │ FFmpeg      │
│ files       │     │ metadata    │     │ BPP check   │     │ GPU/CPU     │
│             │     │             │     │             │     │             │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                                               │                   │
                                               ▼                   ▼
                                        ┌─────────────┐     ┌─────────────┐
                                        │   SKIP      │     │   VERIFY    │
                                        │             │     │             │
                                        │ Already     │     │ VMAF score  │
                                        │ optimized   │     │ Size check  │
                                        │             │     │             │
                                        └─────────────┘     └─────────────┘
```

### Technology Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| **Runtime** | Rust + Tokio | Async, safe, fast |
| **Web Framework** | Axum | High-performance HTTP |
| **Database** | SQLite + SQLx | Embedded, zero-config |
| **Frontend** | Astro + React | Modern, reactive UI |
| **Styling** | Tailwind CSS | Utility-first CSS |
| **Media** | FFmpeg + FFprobe | Industry standard |
| **Packaging** | rust-embed | Single binary deployment |

---

## Architecture

```
┌───────────────────────────────────────────────────────────────┐
│                         Alchemist                             │
├───────────────────────────────────────────────────────────────┤
│      ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│      │   Scanner   │  │  Analyzer   │  │  Processor  │        │
│      │             │  │             │  │             │        │
│      │ • Directory │  │ • FFprobe   │  │ • FFmpeg    │        │
│      │   walking   │  │ • BPP calc  │  │ • GPU/CPU   │        │
│      │ • Filtering │  │ • Decision  │  │ • VMAF      │        │
│      └──────┬──────┘  └──────┬──────┘  └──────┬──────┘        │
│             │                │                │               │
│             └────────────────┼────────────────┘               │
│                              ▼                                │  
│  ┌─────────────────────────────────────────────────────────┐  │
│  │                    Scheduler                            │  │
│  │  • Job queue management                                 │  │
│  │  • Concurrency control                                  │  │
│  │  • Priority ordering                                    │  │
│  └─────────────────────────────────────────────────────────┘  │
│                              │                                │
│                              ▼                                │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │                    SQLite Database                      │  │
│  │  • Jobs, Decisions, Encode Stats                        │  │
│  │  • Users, Sessions                                      │  │
│  └─────────────────────────────────────────────────────────┘  │
│                              │                                │
│                              ▼                                │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │                    Axum Web Server                      │  │
│  │  • REST API endpoints                                   │  │
│  │  • Server-Sent Events (SSE)                             │  │
│  │  • Static file serving                                  │  │
│  └─────────────────────────────────────────────────────────┘  │
└───────────────────────────────────────────────────────────────┘
```

### Module Overview

| Module | File | Purpose |
|--------|------|---------|
| **Main** | `src/main.rs` | Entry point, CLI parsing, server startup |
| **Config** | `src/config.rs` | Configuration loading and validation |
| **Database** | `src/db.rs` | SQLite operations, job management |
| **Server** | `src/server.rs` | Axum routes, API handlers |
| **Scheduler** | `src/scheduler.rs` | Job queue, concurrency control |
| **Processor** | `src/processor.rs` | Orchestrates analysis → encoding flow |
| **Analyzer** | `src/media/analyzer.rs` | FFprobe wrapper, transcode decisions |
| **FFmpeg** | `src/media/ffmpeg.rs` | FFmpeg wrapper, encoding commands |
| **Watcher** | `src/system/watcher.rs` | File system monitoring |

---

## Installation

### Prerequisites

- **Rust**: 1.75+ (for building from source)
- **FFmpeg**: 5.0+ with hardware acceleration support
- **FFprobe**: Included with FFmpeg
- **Bun**: 1.3.4+ (for web development)

### From Source

```bash
# Clone repository
git clone https://github.com/BrooklynLovesZelda/alchemist.git
cd alchemist

# Install dependencies

cd web && bun install

# Build release binary
cd .. && cargo build --release

# Binary located at ./target/release/alchemist
```

### Docker (Recommended)

```bash
# Pull from GitHub Container Registry
docker pull ghcr.io/bybrooklyn/alchemist:latest

# Or build locally
docker build -t alchemist .
```

### First-Run Setup

On first launch, Alchemist runs an interactive setup wizard to:

1. Create admin user account
2. Configure media directories
3. Set transcoding preferences
4. Detect available hardware encoders

---

## Configuration

Configuration is stored at `~/.openbitdo/config.toml` by default on Linux and macOS, and `./config.toml` elsewhere. Set `ALCHEMIST_CONFIG_PATH` to override the path.
Database is stored at `~/.openbitdo/alchemist.db` by default on Linux and macOS, and `./alchemist.db` elsewhere. Set `ALCHEMIST_DB_PATH` to override the path.
If `ALCHEMIST_CONFIG_MUTABLE=false`, settings/setup endpoints will return HTTP `409` for config write attempts.

### Full Configuration Reference

```toml
#──────────────────────────────────────────────────────────────────
# TRANSCODING SETTINGS
#──────────────────────────────────────────────────────────────────
[transcode]
# Preferred codec: "av1", "hevc", or "h264"
output_codec = "av1"

# Allow fallback to other codecs if preferred is unavailable
allow_fallback = true

# Quality profile: "quality", "balanced", or "speed"
quality_profile = "balanced"

# Minimum size reduction required (0.3 = 30%)
# If output isn't 30% smaller, original is kept
size_reduction_threshold = 0.3

# Minimum bits-per-pixel threshold for re-encoding
# Files already below this are skipped
min_bpp_threshold = 0.1

# Skip files smaller than this (in MB)
min_file_size_mb = 50

# Number of concurrent transcode jobs
concurrent_jobs = 2

# HDR handling: "preserve" (keep HDR) or "tonemap" (convert to SDR)
hdr_mode = "preserve"

# Tonemap algorithm: "hable", "mobius", "reinhard", or "clip"
tonemap_algorithm = "hable"

# Tonemap peak luminance (nits)
tonemap_peak = 100

# Tonemap desaturation (0.0 - 1.0)
tonemap_desat = 0.2

#──────────────────────────────────────────────────────────────────
# HARDWARE ACCELERATION
#──────────────────────────────────────────────────────────────────
[hardware]
# Enable CPU fallback when GPU unavailable
allow_cpu_fallback = true

# Allow pure CPU encoding (no GPU required)
allow_cpu_encoding = true

# CPU encoding preset: "slow", "medium", "fast", "faster"
cpu_preset = "medium"

#──────────────────────────────────────────────────────────────────
# SCANNER SETTINGS
#──────────────────────────────────────────────────────────────────
[scanner]
# Directories to scan for media files
directories = [
    "/media/movies",
    "/media/tvshows"
]

# File extensions to include
extensions = ["mkv", "mp4", "avi", "mov", "wmv", "flv", "webm"]

# Skip files matching these patterns
exclude_patterns = ["sample", "trailer", "extras"]

#──────────────────────────────────────────────────────────────────
# SERVER SETTINGS
#──────────────────────────────────────────────────────────────────
[server]
# Web server port
port = 3000

# Bind address
host = "0.0.0.0"
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `ALCHEMIST_CONFIG_PATH` | Primary config file path | `~/.openbitdo/config.toml` on Linux/macOS, `./config.toml` elsewhere |
| `ALCHEMIST_CONFIG` | Legacy alias for config path | unset |
| `ALCHEMIST_DB_PATH` | SQLite database file path | `~/.openbitdo/alchemist.db` on Linux/macOS, `./alchemist.db` elsewhere |
| `ALCHEMIST_DATA_DIR` | Legacy data dir fallback for DB (`<dir>/alchemist.db`) | unset |
| `ALCHEMIST_CONFIG_MUTABLE` | Enable/disable runtime config writes | `true` |

---

## Web Interface

The Alchemist web interface is a modern, responsive single-page application built with Astro and React. It provides real-time monitoring and control of the transcoding engine.

### Navigation

The sidebar provides quick access to all sections:

| Icon | Page | Keyboard Shortcut |
|------|------|-------------------|
| 📊 | Dashboard | - |
| 🎬 | Jobs | - |
| 📁 | Library | - |
| 📜 | Logs | - |
| 📈 | Statistics | - |
| 🎨 | Appearance | - |
| ⚙️ | Settings | - |

### Dashboard (`/`)

The main command center for Alchemist. At a glance, see:

**Summary Cards**
- **Active Jobs**: Currently encoding files with live progress
- **Completed**: Total successful transcodes
- **Failed**: Jobs that encountered errors
- **Total Processed**: Lifetime job count

**Recent Activity Feed**
- Last 5 jobs with file name, status, and timestamp
- Color-coded status badges (green=complete, yellow=active, red=failed)
- Click to view job details

**Quick Actions**
- **Scan Now**: Trigger immediate directory scan
- **Pause/Resume**: Control the processing engine

### Jobs (`/jobs`)

Comprehensive job management interface:

**Job Table Features**
- Sortable columns (name, status, progress, date)
- Status filtering (all, active, completed, failed)
- Search by filename
- Pagination for large queues

**Per-Job Actions**
| Action | Description |
|--------|-------------|
| Cancel | Stop active encoding (preserves original) |
| Restart | Re-queue failed job for retry |
| View Details | See full analysis and encoding logs |

**Bulk Actions**
- **Restart All Failed**: Re-queue all failed jobs
- **Clear Completed**: Remove finished jobs from list

### Statistics (`/stats`)

Deep dive into transcoding performance:

**Overview Cards**
- Space Saved (GB + percentage)
- Total Processed
- Encoding Time
- Average VMAF Score

**Daily Activity Chart**
- Bar chart showing jobs completed per day
- Last 30 days of activity
- Hover for detailed tooltips

**Space Efficiency Visualization**
- Visual comparison of input vs output size
- Animated progress bar

**Performance Metrics Grid**
- Average Compression Ratio
- Average Encoding Speed (fps)
- Average Bitrate (kbps)

**Recent Jobs Table**
- Last 10 completed jobs with full metrics
- Columns: File, Input Size, Output Size, Saved %, Ratio, VMAF, Time

### Library (`/library`)

File browser for your configured media directories:

**Features**
- Navigate directory tree
- View file metadata (size, codec, resolution, bitrate)
- Enqueue individual files for transcoding
- See analysis results without encoding
- Filter by extension

### Logs (`/logs`)

Real-time log viewer powered by Server-Sent Events:

**Log Entry Types**
| Type | Color | Description |
|------|-------|-------------|
| INFO | Blue | General status updates |
| SUCCESS | Green | Job completions |
| WARNING | Yellow | Non-fatal issues |
| ERROR | Red | Failures and exceptions |
| DEBUG | Gray | Detailed diagnostics |

**Features**
- Auto-scroll with pause on hover
- Filter by log level
- Filter by job ID
- Search within logs
- Clear log history

### Settings (`/settings`)

Configure the transcoding engine:

**Transcoding Engine**
- **Output Codec**: Toggle between AV1 and HEVC
- **Quality Profile**: Quality / Balanced / Speed
- **Concurrent Jobs**: 1-8 parallel encodes
- **Min Reduction**: Required size savings (%)
- **Min File Size**: Skip files below threshold

**Changes are applied immediately** - no restart required.

### Appearance (`/appearance`)

Customize the UI with 12+ color profiles organized by category:

**Vibrant & Energetic**
- 🟠 **Helios Orange**: Default warm theme
- 🌅 **Sunset**: Warm gradients
- 💜 **Neon**: Electric cyber aesthetic
- 🔴 **Crimson**: Bold red accents

**Cool & Calm**
- 🔵 **Deep Blue**: Navy with cool highlights
- 🌊 **Ocean**: Teal and turquoise
- 🟢 **Emerald**: Rich green tones

**Soft & Dreamy**
- 💜 **Lavender**: Soft pastels
- 🟣 **Purple**: Velvet violets

**Dark & Minimal**
- ⚫ **Midnight**: Pure OLED black
- ⬛ **Monochrome**: Neutral grayscale
- 🧛 **Dracula**: Classic dev theme

---

## API Reference

All API endpoints require authentication via Bearer token, except for:
- `/api/setup/*` - Setup wizard endpoints
- `/api/auth/login` - Login endpoint
- `/api/health` - Health check (returns status, version, uptime)
- `/api/ready` - Readiness check (returns database status)

### Authentication

```bash
# Login
POST /api/auth/login
Content-Type: application/json
{"username": "admin", "password": "secret"}

# Response: {"token": "abc123..."}

# Use token in subsequent requests
Authorization: Bearer abc123...
```

### Jobs

```bash
# Get all jobs
GET /api/jobs/table

# Get job stats
GET /api/stats

# Cancel job
POST /api/jobs/:id/cancel

# Restart job
POST /api/jobs/:id/restart

# Restart all failed
POST /api/jobs/restart-failed

# Clear completed
POST /api/jobs/clear-completed
```

### Statistics

```bash
# Aggregated stats (totals)
GET /api/stats/aggregated
# Response: {total_input_bytes, total_output_bytes, total_savings_bytes, 
#            total_time_seconds, total_jobs, avg_vmaf}

# Daily stats (last 30 days)
GET /api/stats/daily
# Response: [{date, jobs_completed, bytes_saved, total_input_bytes, 
#             total_output_bytes}, ...]

# Detailed per-job stats
GET /api/stats/detailed
# Response: [{job_id, input_path, input_size_bytes, output_size_bytes,
#             compression_ratio, encode_time_seconds, encode_speed,
#             avg_bitrate_kbps, vmaf_score, created_at}, ...]
```

### Engine Control

```bash
# Pause processing
POST /api/engine/pause

# Resume processing
POST /api/engine/resume

# Get engine status
GET /api/engine/status
```

### Settings

```bash
# Get transcode settings
GET /api/settings/transcode

# Update transcode settings
POST /api/settings/transcode
Content-Type: application/json
{
  "concurrent_jobs": 2,
  "size_reduction_threshold": 0.3,
  "min_bpp_threshold": 0.1,
  "min_file_size_mb": 50,
  "output_codec": "av1",
  "allow_fallback": true,
  "quality_profile": "balanced",
  "threads": 0,
  "hdr_mode": "preserve",
  "tonemap_algorithm": "hable",
  "tonemap_peak": 100,
  "tonemap_desat": 0.2
}
```

### Server-Sent Events

```bash
# Real-time event stream
GET /api/events?token=<auth_token>

# Event types:
# - JobStateChanged: {job_id, status}
# - Progress: {job_id, percentage, time}
# - Decision: {job_id, action, reason}
# - Log: {job_id, message}
```

---

## Database Schema

Alchemist uses SQLite for persistence. The database file is located at `data/alchemist.db`.

### Tables

#### `jobs`
| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `input_path` | TEXT | Source file path (unique) |
| `output_path` | TEXT | Destination file path |
| `status` | TEXT | Job state (queued/encoding/completed/failed/etc) |
| `mtime_hash` | TEXT | File modification time hash |
| `priority` | INTEGER | Job priority (higher = first) |
| `progress` | REAL | Encoding progress 0-100 |
| `attempt_count` | INTEGER | Retry count |
| `created_at` | DATETIME | Job creation time |
| `updated_at` | DATETIME | Last status update |

#### `encode_stats`
| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `job_id` | INTEGER | Foreign key to jobs |
| `input_size_bytes` | INTEGER | Original file size |
| `output_size_bytes` | INTEGER | Encoded file size |
| `compression_ratio` | REAL | input/output ratio |
| `encode_time_seconds` | REAL | Total encoding time |
| `encode_speed` | REAL | Frames per second |
| `avg_bitrate_kbps` | REAL | Output bitrate |
| `vmaf_score` | REAL | Quality score (0-100) |
| `created_at` | DATETIME | Completion time |

#### `decisions`
| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `job_id` | INTEGER | Foreign key to jobs |
| `action` | TEXT | Action taken (encode/skip/revert) |
| `reason` | TEXT | Human-readable explanation |
| `created_at` | DATETIME | Decision time |

#### `users`
| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `username` | TEXT | Unique username |
| `password_hash` | TEXT | Argon2 password hash |
| `created_at` | DATETIME | Account creation time |

#### `sessions`
| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER | Primary key |
| `user_id` | INTEGER | Foreign key to users |
| `token` | TEXT | Session token (unique) |
| `created_at` | DATETIME | Session start |
| `expires_at` | DATETIME | Session expiration |

---

## Database Migration Policy

> **Baseline Version: 0.2.5**

All database migrations maintain **backwards compatibility** with the v0.2.5 schema. This means:

- ✅ Newer app versions work with older database files
- ✅ Database files can be safely upgraded
- ✅ No data loss during upgrades

### Migration Rules

#### Allowed Operations
- Add new tables with `CREATE TABLE IF NOT EXISTS`
- Add new columns with `NULL` or `DEFAULT` values
- Add new indexes with `CREATE INDEX IF NOT EXISTS`
- Insert new configuration rows

#### Forbidden Operations
- Never remove columns
- Never rename columns
- Never change column types
- Never remove tables
- Never add `NOT NULL` columns without defaults

### Schema Version Tracking

The `schema_info` table tracks compatibility:

```sql
SELECT value FROM schema_info WHERE key = 'min_compatible_version';
-- Returns: "0.2.5"
```

---

## Hardware Acceleration

Alchemist auto-detects available hardware encoders at startup.

### Supported Encoders

| GPU | Encoder | Codec Support |
|-----|---------|---------------|
| **NVIDIA** | NVENC | AV1, HEVC |
| **Intel** | QSV | AV1, HEVC |
| **AMD** | VAAPI | HEVC (AV1 limited) |
| **Apple** | VideoToolbox | HEVC (experimental) |
| **CPU** | libsvtav1/x265 | AV1, HEVC |

### Detection Order

1. NVIDIA NVENC (if CUDA available)
2. Intel QuickSync (if iGPU available)
3. AMD VAAPI (on Linux)
4. CPU fallback (always available)

### Docker GPU Passthrough

See [GPU_PASSTHROUGH.md](GPU_PASSTHROUGH.md) for detailed instructions on:
- NVIDIA Container Toolkit setup
- Intel QSV passthrough
- AMD ROCm configuration

---

## Docker Deployment

Published container images are multi-arch for Linux `amd64` and `arm64`. Stable tags publish `:latest`, branch builds publish `:edge`, and every build also gets an exact version tag plus a `sha-<short>` tag.

### Basic Run

```bash
docker run -d \
  --name alchemist \
  -p 3000:3000 \
  -v /path/to/media:/media \
  -v alchemist_data:/app/data \
  ghcr.io/bybrooklyn/alchemist:latest
```

### With NVIDIA GPU

```bash
docker run -d \
  --name alchemist \
  --gpus all \
  -p 3000:3000 \
  -v /path/to/media:/media \
  -v alchemist_data:/app/data \
  ghcr.io/bybrooklyn/alchemist:latest
```

### Docker Compose

```yaml
version: "3.8"
services:
  alchemist:
    image: ghcr.io/bybrooklyn/alchemist:latest
    container_name: alchemist
    restart: unless-stopped
    ports:
      - "3000:3000"
    volumes:
      - /path/to/media:/media
      - alchemist_data:/app/data
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]

volumes:
  alchemist_data:
```

### Image Tags

| Tag | Description |
|-----|-------------|
| `latest` | Latest stable release |
| `edge` | Latest `main` / `master` branch build |
| `0.2.10-rc.1` | Exact version or prerelease |
| `sha-<short>` | Immutable commit-based image |

---

## Development

This section covers setting up a development environment and contributing to Alchemist.

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | 1.75+ | Backend compilation |
| Bun | 1.3.4+ | Frontend package management |
| FFmpeg | 5.0+ | Media processing (runtime) |
| Docker | 20.10+ | Container builds (optional) |

### Project Structure

```
alchemist/
├── src/                       # Rust backend
│   ├── main.rs               # Entry point & CLI
│   ├── config.rs             # Configuration loading
│   ├── db.rs                 # Database operations (600+ lines)
│   ├── server.rs             # Axum routes & handlers
│   ├── scheduler.rs          # Job queue management
│   ├── processor.rs          # Transcode orchestration
│   ├── error.rs              # Error types & handling
│   ├── media/
│   │   ├── mod.rs            # Module exports
│   │   ├── analyzer.rs       # FFprobe wrapper & decisions
│   │   └── ffmpeg.rs         # FFmpeg command builder
│   └── system/
│       ├── mod.rs            # Module exports
│       └── watcher.rs        # File system monitoring
│
├── web/                       # Frontend (Astro + React)
│   ├── src/
│   │   ├── pages/            # Astro page routes
│   │   │   ├── index.astro   # Dashboard
│   │   │   ├── jobs.astro    # Job management
│   │   │   ├── stats.astro   # Statistics dashboard
│   │   │   └── ...
│   │   ├── components/       # React components
│   │   │   ├── Dashboard.tsx
│   │   │   ├── StatsCharts.tsx
│   │   │   ├── SystemStatus.tsx
│   │   │   └── ...
│   │   ├── layouts/          # Page layouts
│   │   ├── lib/              # Utilities
│   │   │   └── api.ts        # Authenticated fetch
│   │   └── styles/           # Global CSS
│   ├── astro.config.mjs
│   ├── tailwind.config.mjs
│   └── package.json
│
├── migrations/                # SQL migrations
│   ├── 20231026000000_initial_schema.sql
│   ├── 20240109120000_add_auth_tables.sql
│   └── MIGRATIONS.md         # Migration policy
│
├── docs/                      # Documentation
│   ├── Documentation.md      # This file
│   └── GPU_PASSTHROUGH.md    # GPU setup guide
│
├── .github/
│   └── workflows/
│       ├── ci.yml            # Rust + frontend validation
│       ├── docker.yml        # Multi-arch container publishing
│       └── release.yml       # Standalone binary release pipeline
│
├── Cargo.toml                # Rust dependencies
├── Cargo.lock
├── Dockerfile                # Container build
├── VERSION                   # Version tracking
├── README.md
└── config.toml               # Runtime config
```

### Building

#### Backend (Rust)

```bash
# Development build (fast compile, debug symbols)
cargo build

# Release build (optimized, slower compile)
cargo build --release

# Check for errors without building
cargo check

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run -- 

# Run in production mode
./target/release/alchemist
```

#### CLI Mode

```bash
# Process directories in CLI mode
./target/release/alchemist --cli --dir /path/to/videos --dir /another/path

# Dry run (analyze only)
./target/release/alchemist --cli --dry-run --dir /path/to/videos
```

#### Reset Auth

```bash
# Clear users/sessions and re-run setup
./target/release/alchemist --reset-auth
```

#### Frontend (Astro + React)

```bash
cd web

# Install dependencies with Bun
bun install --frozen-lockfile

# Development server with hot reload (port 4321)
bun run dev

# Type check
bun run typecheck

# Astro diagnostics
bun run check

# Production build (outputs to dist/)
bun run build

# Preview production build
bun run preview
```

#### Docker Build

```bash
# Full Docker build
docker build -t alchemist .

# Build with no cache (clean build)
docker build --no-cache -t alchemist .

# Build a specific platform image locally
docker build --platform linux/amd64 -t alchemist .

# Build multi-arch images with Buildx
docker buildx build --platform linux/amd64,linux/arm64 -t alchemist .
```

### Code Style

**Rust**
- Follow standard Rust formatting: `cargo fmt`
- Run clippy for lints: `cargo clippy`
- Use `Result<T>` for fallible operations
- Prefer `anyhow` for error handling

**TypeScript/React**
- Use functional components with hooks
- Type all props with interfaces
- Use the `apiFetch` utility for API calls
- Follow Tailwind class ordering conventions

### Adding a New API Endpoint

1. **Add route** in `src/server.rs`:

   ```rust
   .route("/api/my/endpoint", get(my_handler))
   ```

2. **Implement handler**:

   ```rust
   async fn my_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
       // Your logic here
       axum::Json(serde_json::json!({"ok": true}))
   }
   ```

3. **Update documentation** in `docs/Documentation.md`

### Adding a New Database Table

1. **Create migration** in `migrations/`:

   ```sql
   -- migrations/20260110000000_add_my_table.sql
   CREATE TABLE IF NOT EXISTS my_table (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       name TEXT NOT NULL,
       created_at DATETIME DEFAULT CURRENT_TIMESTAMP
   );
   ```

2. **Add struct** in `src/db.rs`:

   ```rust
   #[derive(Debug, Serialize, Deserialize)]
   pub struct MyRecord {
       pub id: i64,
       pub name: String,
       pub created_at: DateTime<Utc>,
   }
   ```

3. **Follow migration policy** (see `migrations/MIGRATIONS.md`)

### Versioning

Version metadata is tracked in three files that must stay in sync:
- `VERSION` file (read by Docker workflow)
- `Cargo.toml` `version` field
- `web/package.json` `version` field

Supporting release metadata must also be updated for every cut:
- `CHANGELOG.md` top release entry
- `docs/Documentation.md` changelog section and footer
- `Cargo.lock` root package version should be refreshed after the Cargo version bump

Recommended workflow:
1. Run `./scripts/bump_version.sh 0.2.10-rc.1`
2. Update `CHANGELOG.md` and the docs changelog entry with release notes
3. Run `cargo fmt --all -- --check`
4. Run `cargo clippy --all-targets --all-features -- -D warnings`
5. Run `cargo test --all-targets`
6. Run `bun install --frozen-lockfile && bun run typecheck && bun run build` in `web/`
7. Let `.github/workflows/ci.yml` validate the branch or pull request before merge
8. Push the merged release commit to `main` or `master` so `.github/workflows/docker.yml` publishes the exact version tag and, for stable builds, `:latest`
9. Use `workflow_dispatch` on `.github/workflows/release.yml` as a dry run if you want to exercise the release matrix without publishing assets
10. Create annotated tag `v0.2.10-rc.1` on that exact merged commit to publish Linux (`.tar.gz`), Windows (`.exe`), and macOS (`.tar.gz`) release assets with SHA256 files

Important:
- CI runs on pushes to `main` / `master`, pull requests, and manual dispatches, while docs-only changes are ignored
- Docker publishing is driven by `VERSION` plus source changes on push to `main` / `master`
- Standalone release artifacts are driven by the `v*` git tag
- GitHub releases from `-rc.` tags must be marked prerelease and must not become latest
- Linux and macOS releases are plain `.tar.gz` archives containing the server binary, not desktop bundles
- Do not create the release tag from a dirty or unmerged worktree

---

## Troubleshooting

### Common Issues

#### "Failed to load settings" / 401 Unauthorized

**Cause**: Auth token not being sent with API requests.

**Solution**: Clear browser localStorage and re-login:
```javascript
localStorage.removeItem('alchemist_token');
window.location.href = '/login';
```

#### "No hardware encoder detected"

**Cause**: GPU drivers or container configuration issue.

**Solution**:
1. Verify FFmpeg has hardware support: `ffmpeg -encoders | grep -E "nvenc|qsv|vaapi"`
2. For Docker, ensure GPU passthrough is configured (see GPU_PASSTHROUGH.md)
3. Check driver installation on host

#### Jobs stuck in "queued" state

**Cause**: Engine paused or scheduler not running.

**Solution**:
1. Check engine status: `GET /api/engine/status`
2. Resume if paused: `POST /api/engine/resume`
3. Check logs for errors

#### High CPU usage during encoding

**Cause**: CPU fallback active instead of GPU.

**Solution**:
1. Verify GPU encoder available
2. Check `concurrent_jobs` setting (reduce if needed)
3. Consider using "speed" quality profile

#### Database locked errors

**Cause**: Multiple processes accessing SQLite simultaneously.

**Solution**:
1. Ensure only one Alchemist instance is running
2. Check for zombie processes: `ps aux | grep alchemist`
3. Delete lock file if stuck: `rm data/*.lock`

#### Files being skipped unexpectedly

**Cause**: Analysis determined transcode wouldn't be beneficial.

**Solution**:
1. Check the decision log in `/logs` for the reason
2. Common reasons:
   - File already has low BPP (already optimized)
   - File smaller than `min_file_size_mb`
   - Estimated savings below threshold
3. Adjust thresholds in Settings if needed

#### Output files larger than original

**Cause**: This shouldn't happen - Alchemist should auto-revert.

**Solution**:
1. Check `size_reduction_threshold` setting (default 30%)
2. If revert failed, check disk space
3. Report as bug if persists

#### VMAF scores not appearing

**Cause**: VMAF calculation requires libvmaf in FFmpeg.

**Solution**:
1. Check FFmpeg has VMAF: `ffmpeg -filters | grep vmaf`
2. Install FFmpeg with VMAF support
3. VMAF is optional - encoding works without it

#### "Connection refused" on port 3000

**Cause**: Server not running or bound to different address.

**Solution**:
1. Check if process is running: `ps aux | grep alchemist`
2. Check bound address in config (default `0.0.0.0:3000`)
3. Check firewall rules
4. For Docker: ensure port is mapped (`-p 3000:3000`)

### Log Locations

| Environment | Path | Command |
|-------------|------|---------|
| Binary | stdout/stderr | Redirect with `2>&1` |
| Docker | Container logs | `docker logs alchemist` |
| Docker (follow) | Live logs | `docker logs -f alchemist` |
| Systemd | Journal | `journalctl -u alchemist` |
| Systemd (follow) | Live journal | `journalctl -fu alchemist` |

### Debug Mode

Run with verbose logging to diagnose issues:

```bash
# Binary
RUST_LOG=debug ./alchemist

# Docker
docker run -e RUST_LOG=debug ...
```

Log levels: `error`, `warn`, `info`, `debug`, `trace`

---

## FAQ

### General

**Q: What video formats does Alchemist support?**

A: Input: Any format FFmpeg can read (MKV, MP4, AVI, MOV, WMV, FLV, WebM, etc.)
   Output: Same container as input, with AV1 or HEVC video codec.

**Q: Will transcoding affect audio or subtitles?**

A: No. Audio and subtitle streams are copied unchanged (`-c:a copy -c:s copy`).

**Q: How long does transcoding take?**

A: Depends on file size, resolution, and encoder:
- GPU (NVENC): ~50-300 fps for 1080p
- CPU (SVT-AV1): ~5-30 fps for 1080p

A 2-hour 1080p movie takes roughly 30min (GPU) or 3-4 hours (CPU).

**Q: Does Alchemist delete original files?**

A: Only after successful transcode AND if the output meets quality/size thresholds. Originals are preserved until fully validated.

### Quality

**Q: What VMAF score is acceptable?**

A: 
- 95+: Virtually identical to original
- 90-95: Imperceptible quality loss
- 85-90: Minor loss, acceptable for archival
- <85: Noticeable degradation

Alchemist targets 93+ by default.

**Q: What's the difference between quality profiles?**

| Profile | Speed | Size | Quality |
|---------|-------|------|---------|
| Quality | Slowest | Smallest | Best |
| Balanced | Medium | Medium | Good |
| Speed | Fastest | Largest | Acceptable |

**Q: AV1 vs HEVC - which should I use?**

| Factor | AV1 | HEVC |
|--------|-----|------|
| Compression | 20-30% better | Good |
| Encode Speed | Slower | Faster |
| Hardware Support | Newer GPUs only | Wide support |
| Playback | Modern devices | Almost everything |

Use AV1 if you have compatible hardware. Use HEVC for maximum compatibility.

### Storage & Performance

**Q: How much space will I save?**

A: Typical savings:
- Old H.264 content: 40-60% smaller
- Already optimized content: 10-20% or skipped
- 4K HDR: 30-50% smaller

**Q: Will transcoding impact my system performance?**

A: 
- GPU encoding: Minimal CPU impact, GPU at 50-100%
- CPU encoding: High CPU usage (configurable via `concurrent_jobs`)
- RAM: ~500MB per concurrent job
- Disk I/O: Read/write proportional to file size

**Q: How do I limit resource usage?**

Set `concurrent_jobs = 1` in settings for minimal system impact.

### Docker

**Q: How do I persist data across container restarts?**

A: Mount a volume for `/app/data`:
```bash
docker run -v alchemist_data:/app/data ...
```

**Q: Container can't see my media files?**

A: Mount your media directory:
```bash
docker run -v /path/on/host:/media ...
```

**Q: How do I update to a new version?**

```bash
docker pull ghcr.io/bybrooklyn/alchemist:latest
docker stop alchemist
docker rm alchemist
docker run ... # same options as before
```

### Technical

**Q: Does Alchemist support HDR?**

A: Yes, HDR metadata is preserved during transcoding.

**Q: Can I run multiple instances?**

A: Not recommended. Use a single instance with multiple scan directories.

**Q: Is there an API rate limit?**

A: No rate limiting on the local API. All endpoints respond instantly.

**Q: How does authentication work?**

A: 
1. Login with username/password → receive JWT token
2. Token stored in localStorage
3. Token sent as `Authorization: Bearer <token>` header
4. Tokens expire after 7 days

---

## Changelog

### v0.2.10-rc.1
- ✅ Job lifecycle safety pass: queued vs active cancel handling, active-job delete/restart blocking, batch conflict responses, and stricter DB persistence checks
- ✅ Output safety upgrade: `output_root`, mirrored destination paths, temp-file promotion, and non-destructive replace flow
- ✅ Scheduler/watch/setup parity: immediate schedule refresh, Intel Arc H.264 detection fix, H.264 setup option, canonicalized watch folders, and recursive watch-folder controls
- ✅ Jobs/settings UX improvements: per-job priority controls, output-root file settings, active-job-safe actions, and Astro router deprecation cleanup
- ✅ CI/CD rewrite: cached Rust checks, frontend typecheck/build validation, multi-arch Docker publishing, and unified release metadata flow
- ✅ Release packaging simplified: Linux and macOS now ship plain `.tar.gz` binaries, Windows ships `.exe`, and every asset includes SHA256 checksums

### v0.2.8
- ✅ Default server mode; explicit CLI with `--cli --dir ...` and new `--reset-auth` flow
- ✅ Login redirects to setup when no users exist
- ✅ Startup banner now includes version/build info
- ✅ Telemetry reliability: shared client, timeout, retry/backoff, and speed sanitization
- ✅ Dashboard redesign, spacing polish, dynamic Quick Start, and mobile responsiveness
- ✅ Sidebar brand/logo now links to dashboard
- ✅ Settings nav auto-scrolls to active tab; section separators removed
- ✅ Setup wizard: telemetry toggle removed; CPU encoding defaults on
- ✅ Added 10 new themes, including dark mint (`Mint Night`)
- ✅ Release pipeline: Windows EXE-only; macOS app ad-hoc signed with improved Info.plist
- ✅ VideoToolbox quality tuning + HEVC `hvc1` tagging; AV1→HEVC fallback for Apple
- ✅ HDR metadata detection, tone mapping controls, and color metadata preservation
- ✅ Transcode settings expanded: HDR/tonemap options + fallback policy
- ✅ Codec/encoder negotiation: AV1 is preference with hardware/CPU fallback chain
- ✅ FFmpeg encoder cache probed at startup; no hard assumptions about encoder availability
- ✅ Planner logic refined: H.264 always transcodes; BPP gate only when bitrate/fps are known
- ✅ Jobs UI upgraded: in‑app confirm modals, contextual action menu, retry/cancel/delete polish

### v0.2.6-2
- ✅ Setup wizard now authenticates scan and hardware calls to prevent endless loading
- ✅ Scheduler window validation and normalized time handling
- ✅ File watcher no longer blocks on bursty filesystem events
- ✅ DB stability pass: WAL + busy timeout + foreign keys enabled
- ✅ Legacy watch directory schemas now supported at runtime
- ✅ Session cleanup task to prevent DB growth
- ✅ New DB indexes for faster jobs/logs/schedule/notifications queries
- ✅ Reqwest switched to rustls for cross-compiles without OpenSSL
- ✅ Cross-platform build script (bun + zig + cargo-xwin)
- ✅ Design philosophy added for consistent development standards

### v0.2.5
- ✅ Async runtime reliability improvements (spawn_blocking for ffprobe/VMAF/hardware detection)
- ✅ Accurate encode_speed and avg_bitrate_kbps metrics computed from actual media duration
- ✅ GPU utilization monitoring in dashboard
- ✅ Public /api/health and /api/ready endpoints for container orchestration
- ✅ CPU encoding toggle in hardware settings
- ✅ Output path refresh on config changes

### v0.2.3
- ✅ Database migration policy (backwards compatible from this version)
- ✅ Schema version tracking
- ✅ Auth token injection for API calls
- ✅ Statistics dashboard with charts
- ✅ Docker workflow improvements

### v0.2.2
- Initial public release
- Core transcoding engine
- Web dashboard
- Multi-theme support

---

## License

Alchemist is licensed under the **GPL-3.0 License**. See `LICENSE` for details.

---

## Getting Help

- **GitHub Issues**: [github.com/BrooklynLovesZelda/alchemist/issues](https://github.com/bybrooklyn/alchemist/issues)
- **Logs**: Use `/logs` in web UI for real-time diagnostics
- **Documentation**: You're here! 📖

---

*Documentation for Alchemist v0.2.10-rc.1*
