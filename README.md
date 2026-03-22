# Alchemist

Point it at your media library. Walk away. Come back to a smaller, better-encoded collection.

Alchemist saves space automatically without asking you to babysit shell commands or risk your originals. It is free, open source, self-hosted, and gives you a web UI for setup, monitoring, and day-to-day control. If you want storage back without turning media management into a hobby project, this is the tool.

## Why Alchemist?

Running `ffmpeg` by hand works when you only have a few files and a lot of patience. Tools like Tdarr can scale, but they also ask you to learn a larger system. Alchemist is built for the middle: no plugin stacks, no flow editors, no separate services to install, smart enough to skip files that would not really benefit, shipped as a single binary, genuinely GPLv3 open source instead of source-available, and designed for people who just want it to work.

## What It Does

Alchemist scans your library, inspects each file, and decides whether transcoding would actually help. If a file is already efficient, it skips it and tells you why in plain English instead of leaving you to guess.

If supported hardware is available, Alchemist uses it automatically. NVIDIA, Intel, AMD, and Apple Silicon are all detected and used without manual setup, and if there is no GPU available it falls back to CPU encoding on its own.

Your originals stay safe. Alchemist never overwrites anything until the new file passes its quality checks. You can keep both files or let Alchemist replace the original, but nothing is lost until you decide that is what you want.

Everything is visible in the web dashboard. You can see what is running, what was skipped, how much space you have recovered, and pause or cancel work whenever you want.

## Features

- Give movies, TV, and home videos different behavior with per-library profiles.
- Catch corrupt or broken files before they surprise you with Library Doctor.
- See exactly how much storage you have recovered in the savings dashboard.
- Understand every skipped file immediately with plain-English explanations.
- Get a ping when work finishes through Discord, Gotify, or a webhook.
- Keep heavy jobs out of the way with a scheduler for off-peak hours.
- Push urgent files to the front with the priority queue.
- Let hardware acceleration happen automatically on NVIDIA, Intel, AMD, or Apple, with CPU fallback when needed.
- Preserve HDR metadata or tonemap to SDR depending on what you need.
- Add folders once and let watch folders keep monitoring them automatically.

## Hardware Support

Alchemist uses hardware acceleration when it can and falls back to CPU encoding automatically when it cannot. You do not need different workflows for different machines.

| Vendor | Encoders |
|--------|----------|
| NVIDIA | AV1, HEVC, H.264 (NVENC) |
| Intel  | AV1, HEVC, H.264 (QSV) |
| AMD    | HEVC, H.264 (VAAPI/AMF) |
| Apple  | HEVC, H.264 (VideoToolbox) |
| CPU    | AV1 (SVT-AV1), HEVC (x265), H.264 (x264) |

CPU fallback is automatic when no GPU is available.

## Quick Start

### Docker (Recommended)

If you want the fastest path to a running instance, use the published container:

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

Then open [http://localhost:3000](http://localhost:3000) in your browser.

If you prefer `docker run`, this is the trimmed equivalent:

```bash
docker run -d --name alchemist -p 3000:3000 -v /path/to/config:/app/config -v /path/to/data:/app/data -v /path/to/media:/media -e ALCHEMIST_CONFIG_PATH=/app/config/config.toml -e ALCHEMIST_DB_PATH=/app/data/alchemist.db --restart unless-stopped ghcr.io/bybrooklyn/alchemist:latest
```

### Binary

Download the latest release from [GitHub Releases](https://github.com/bybrooklyn/alchemist/releases). Prebuilt binaries are published for Linux x86_64, Linux ARM64, Windows x86_64, macOS Intel, and macOS Apple Silicon.

FFmpeg must be installed separately for binary installs:

```bash
sudo apt install ffmpeg
sudo dnf install ffmpeg
sudo pacman -S ffmpeg
brew install ffmpeg
winget install Gyan.FFmpeg
```

Start Alchemist, then open [http://localhost:3000](http://localhost:3000):

```bash
./alchemist
```

On Windows, run `alchemist.exe` instead.

### From Source

```bash
git clone https://github.com/bybrooklyn/alchemist.git
cd alchemist
cargo build --release
./target/release/alchemist
```

`Cargo.toml` does not currently declare a `rust-version`, so the project does not publish a pinned MSRV yet. Use the current stable Rust toolchain, and make sure FFmpeg is installed separately.

## First Run

1. Open [http://localhost:3000](http://localhost:3000).
2. Complete the setup wizard. It takes about 2 minutes.
3. Add your media folders in Watch Folders.
4. Alchemist scans and starts working automatically.
5. Check the Dashboard to see progress and savings.

## Supported Platforms

| Platform | Status |
|----------|--------|
| Linux x86_64 | ✅ Supported |
| Linux ARM64 | ✅ Supported |
| Windows x86_64 | ✅ Supported |
| macOS Apple Silicon | ✅ Supported |
| macOS Intel | ✅ Supported |
| Docker linux/amd64 | ✅ Supported |
| Docker linux/arm64 | ✅ Supported |

## License

Licensed under GPLv3. See [LICENSE](LICENSE) for details.
