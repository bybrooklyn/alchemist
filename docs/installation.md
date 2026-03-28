---
title: Installation
description: Install Alchemist via Docker, binary, or from source.
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
      - /path/to/config:/app/config
      - /path/to/data:/app/data
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
setup wizard runs on first visit.

For GPU passthrough (NVIDIA, Intel, AMD) see
[GPU Passthrough](/gpu-passthrough).

**docker run:**

```bash
docker run -d \
  --name alchemist \
  -p 3000:3000 \
  -v /path/to/config:/app/config \
  -v /path/to/data:/app/data \
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

## From source

```bash
git clone https://github.com/bybrooklyn/alchemist.git
cd alchemist
cd web && bun install --frozen-lockfile && bun run build && cd ..
cargo build --release
./target/release/alchemist
```

Requires Rust 1.85+. Run `rustup update stable` first.

## Nightly builds

```bash
docker pull ghcr.io/bybrooklyn/alchemist:nightly
```

Nightly builds publish on every push to `main` after Rust
checks pass. Use `:latest` for stable releases.
