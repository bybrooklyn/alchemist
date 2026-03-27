---
title: Getting Started
description: Complete guide to installing and setting up Alchemist for the first time.
---

# Getting Started with Alchemist

This guide will walk you through installing Alchemist, completing the setup wizard, and adding your first media library. By the end, you'll have Alchemist automatically transcoding your videos to save storage space.

## Installation Options

Alchemist can be installed in three ways. **Docker is strongly recommended** for the best experience since it includes all necessary dependencies and hardware drivers.

### Docker (Recommended)

Docker provides the smoothest installation experience with automatic hardware detection and all FFmpeg dependencies pre-installed.

#### Docker Compose
Create a `docker-compose.yml` file:

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
      - /path/to/your/media:/media
    environment:
      - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
      - ALCHEMIST_DB_PATH=/app/data/alchemist.db
      - TZ=UTC
    restart: unless-stopped
    # For NVIDIA GPUs:
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
    # For Intel/AMD GPUs on Linux:
    devices:
      - /dev/dri:/dev/dri
```

Then run:
```bash
docker compose up -d
```

#### Docker Run
For a quick one-liner installation:

```bash
docker run -d \
  --name alchemist \
  -p 3000:3000 \
  -v ./config:/app/config \
  -v ./data:/app/data \
  -v /path/to/your/media:/media \
  -e ALCHEMIST_CONFIG_PATH=/app/config/config.toml \
  -e ALCHEMIST_DB_PATH=/app/data/alchemist.db \
  --restart unless-stopped \
  ghcr.io/bybrooklyn/alchemist:latest
```

### Binary Installation

Download pre-built binaries from [GitHub Releases](https://github.com/bybrooklyn/alchemist/releases) for:
- Linux x86_64 and ARM64  
- Windows x86_64
- macOS Intel and Apple Silicon

**Requirements:**
- FFmpeg must be installed separately
- Hardware drivers for GPU acceleration (optional but recommended)

#### Install FFmpeg

**Linux:**
```bash
# Ubuntu/Debian
sudo apt install ffmpeg

# Fedora/RHEL
sudo dnf install ffmpeg

# Arch Linux
sudo pacman -S ffmpeg
```

**macOS:**
```bash
brew install ffmpeg
```

**Windows:**
```bash
winget install Gyan.FFmpeg
```

#### Run Alchemist
```bash
# Linux/macOS
./alchemist

# Windows
alchemist.exe
```

### Build from Source

For developers or users who want the latest features:

```bash
git clone https://github.com/bybrooklyn/alchemist.git
cd alchemist
cargo build --release
./target/release/alchemist
```

**Requirements:**
- Rust toolchain (latest stable)
- FFmpeg installed separately
- Node.js 18+ for building the web interface

## First Run Setup

1. **Access the Web Interface**
   Open http://localhost:3000 in your browser

2. **Complete the Setup Wizard**
   - Set admin password
   - Choose hardware preferences
   - Configure basic transcoding settings
   - This takes about 2-3 minutes

3. **Hardware Detection**
   Alchemist will automatically detect:
   - NVIDIA GPUs (NVENC)
   - Intel integrated graphics (QSV)  
   - AMD graphics (VAAPI/AMF)
   - Apple Silicon (VideoToolbox)
   - Falls back to CPU encoding if no GPU found

## Adding Your First Library

1. **Navigate to Watch Folders**
   Go to Settings → Watch Folders

2. **Add Media Directory**
   - Click "Add Folder"
   - Browse to your media collection
   - Enable "Recursive" to include subdirectories  
   - Enable "Watch Mode" for automatic scanning

3. **Start Processing**
   - Alchemist begins scanning immediately
   - Initial scan shows which files are candidates for transcoding
   - Processing starts automatically based on your settings

## Understanding the Dashboard

Once running, monitor progress from the main dashboard:

- **Active Jobs**: Currently transcoding files
- **Queue**: Files waiting to be processed  
- **Statistics**: Storage saved, files processed
- **System Status**: Hardware usage, temperatures

## Next Steps

- **Configure Profiles**: Set different behaviors for movies vs TV shows
- **Set Schedules**: Limit transcoding to off-peak hours
- **Enable Notifications**: Get alerts when jobs complete
- **Review Quality Settings**: Adjust the balance between size and quality

See the [Configuration Reference](/reference/configuration/) for detailed setting explanations.