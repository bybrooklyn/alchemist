---
title: Install Alchemist — Docker, Binary, or Source
description: Install Alchemist on Linux, macOS, or Windows. Docker Compose is the recommended path; native binaries and source builds are also supported.
keywords:
  - install alchemist
  - self-hosted transcoder install
  - alchemist docker install
---

Alchemist ships as a single binary with the web UI embedded.
The Docker image bundles FFmpeg — nothing else to install.

## Docker (recommended)

**Docker Compose:**

```yaml
services:
  alchemist:
    image: ghcr.io/bybrooklyn/alchemist:latest
    container_name: alchemist
    ports:
      - "3000:3000"
    volumes:
      - ~/.config/alchemist:/app/config
      - ~/.config/alchemist:/app/data
      - /path/to/media:/media
    environment:
      - ALCHEMIST_CONFIG_PATH=/app/config/config.toml
      - ALCHEMIST_DB_PATH=/app/data/alchemist.db
    restart: unless-stopped
```

```bash
docker compose up -d
```

Open [http://localhost:3000](http://localhost:3000). The
setup wizard runs on first visit and is only reachable
from the local network until the first account is created.

For GPU passthrough (NVIDIA, Intel, AMD) see
[GPU Passthrough](/gpu-passthrough).

**docker run:**

```bash
docker run -d \
  --name alchemist \
  -p 3000:3000 \
  -v ~/.config/alchemist:/app/config \
  -v ~/.config/alchemist:/app/data \
  -v /path/to/media:/media \
  -e ALCHEMIST_CONFIG_PATH=/app/config/config.toml \
  -e ALCHEMIST_DB_PATH=/app/data/alchemist.db \
  --restart unless-stopped \
  ghcr.io/bybrooklyn/alchemist:latest
```

## Binary

Download from [GitHub Releases](https://github.com/bybrooklyn/alchemist/releases).
Available for Linux x86_64, Linux ARM64, Windows x86_64,
macOS Apple Silicon, and macOS Intel.

### Package-manager metadata

Release packaging metadata is generated from this repo’s
`packaging/` templates during release publication.

- Homebrew formula source lives under `packaging/homebrew/`
- AUR metadata source lives under `packaging/aur/`

FFmpeg must be installed separately:

```bash
sudo apt install ffmpeg       # Debian / Ubuntu
sudo dnf install ffmpeg       # Fedora
sudo pacman -S ffmpeg         # Arch
brew install ffmpeg           # macOS
winget install Gyan.FFmpeg    # Windows
```

```bash
./alchemist        # Linux / macOS
alchemist.exe      # Windows
```

On Windows, Alchemist now exposes an in-app update check in
the About dialog that compares the running version against
the latest stable GitHub Release and links directly to the
download page when an update is available.

## From source

For macOS and Linux:

```bash
git clone https://github.com/bybrooklyn/alchemist.git
cd alchemist
just install
just build
./target/release/alchemist
```

Requires Rust 1.85+. Run `rustup update stable` first.

For Windows local development:

```bash
just install-w
just dev
```

Windows contributor support covers the core `install/dev/check` path.
Broader `just` release and utility recipes remain Unix-first.

## CLI subcommands

```bash
alchemist scan /path/to/media
alchemist run /path/to/media
alchemist plan /path/to/media
alchemist plan /path/to/media --json
```

- `scan` enqueues matching jobs and exits
- `run` scans, enqueues, and waits for processing to finish
- `plan` reports what Alchemist would do without enqueueing jobs

## Nightly builds

```bash
docker pull ghcr.io/bybrooklyn/alchemist:nightly
```

Nightly builds publish on every push to `main` after Rust
checks pass. Use `:latest` for stable releases.

## See also

- [First Run](/first-run) — setup wizard.
- [Hardware Acceleration](/hardware) — NVENC, Quick Sync,
  VAAPI, AMF, VideoToolbox.
- [Alchemist for Jellyfin](/jellyfin).
- [Alchemist vs Tdarr](/alternatives/tdarr) ·
  [Alchemist vs FileFlows](/alternatives/fileflows).
