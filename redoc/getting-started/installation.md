# Installation

Install Alchemist via Docker, binary, or from source.

Alchemist ships as a single binary with the web UI embedded.
The Docker image also bundles FFmpeg so you don't need to
install it separately.

## Docker (recommended)

### Docker Compose

Create a `docker-compose.yml`:

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

Then start it:

```bash
docker compose up -d
```

  
### docker run

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

  
Open [http://localhost:3000](http://localhost:3000). The
setup wizard will guide you through the rest.

> Tip: For hardware acceleration (NVIDIA, Intel, AMD), you
> need to pass your GPU into the container. See the
> [GPU Passthrough guide](../guides/gpu-passthrough.md).

## Binary

Download the latest release from
[GitHub Releases](https://github.com/bybrooklyn/alchemist/releases).
Prebuilt binaries are available for:

- Linux x86_64 and ARM64
- Windows x86_64
- macOS Apple Silicon and Intel

FFmpeg must be installed separately for binary installs:

```bash
# Debian / Ubuntu
sudo apt install ffmpeg

# Fedora
sudo dnf install ffmpeg

# Arch
sudo pacman -S ffmpeg

# macOS
brew install ffmpeg

# Windows
winget install Gyan.FFmpeg
```

Start Alchemist:

```bash
./alchemist          # Linux / macOS
alchemist.exe        # Windows
```

Then open [http://localhost:3000](http://localhost:3000).

## From source

```bash
git clone https://github.com/bybrooklyn/alchemist.git
cd alchemist
# Build frontend first
cd web && bun install --frozen-lockfile && bun run build && cd ..
# Build the binary
cargo build --release
./target/release/alchemist
```

Alchemist requires Rust 1.85 or later. Run
`rustup update stable` to ensure you are on a recent toolchain.
FFmpeg must be installed separately.

## Nightly builds

Nightly builds are published on every push to `main` after
Rust checks pass:

```bash
docker pull ghcr.io/bybrooklyn/alchemist:nightly
```

Nightly builds include the short commit hash in the version
string (e.g. `0.3.0-dev.3-nightly+abc1234`).
