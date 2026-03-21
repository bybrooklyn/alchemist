# Alchemist

> Intelligent video transcoding automation with hardware acceleration and CPU fallback

Alchemist is a Rust-based video transcoding system that automatically converts your media library to efficient modern codecs using hardware acceleration (GPU) or software encoding (CPU fallback).

## Features

- **Hardware Acceleration**: Supports NVIDIA (NVENC), Intel (QSV), AMD (VAAPI), Apple (VideoToolbox)
- **Codec Targets**: AV1, HEVC, and H.264 output profiles
- **CPU Fallback**: Automatic software encoding when GPU acceleration is unavailable
- **Intelligent Analysis**: Only transcodes files that will benefit from compression
- **Web Dashboard**: Axum backend with an Astro + React frontend for real-time monitoring and control
- **Single Binary**: All assets and templates are embedded into the binary for easy deployment
- **Background Processing**: Queue-based system with concurrent job support
- **Performance Metrics**: Detailed logging and statistics for each transcode job
- **HDR Handling**: Preserve HDR metadata or tonemap to SDR for compatibility
- **Priority Queueing**: Promote, lower, or reset per-job priority from the dashboard
- **Watch Folders**: Mix recursive and top-level watch directories
- **Safe Replace Flow**: Replacement encodes write to a temporary file and promote only after all gates pass
- **Mirrored Output Roots**: Optionally write outputs to a separate root while preserving source-relative folders

## Quick Start

### Prerequisites

- **Rust**: 1.75+
- **FFmpeg**: With hardware acceleration support and libsvtav1
- **FFprobe**: For media analysis

### Installation

```bash
# Clone the repository
git clone https://github.com/BrooklynLovesZelda/alchemist.git
cd alchemist

# Build the project
cargo build --release

# Run the server (default)
./target/release/alchemist
```

### Docker Deployment

```bash
# Build the Docker image
docker build -t alchemist .

# Run the container
docker run -d \
  -p 3000:3000 \
  -v /path/to/config.toml:/app/config/config.toml:ro \
  -v /path/to/media:/media \
  -v /path/to/output:/output \
  -v /path/to/data:/app/data \
  -e ALCHEMIST_CONFIG_PATH=/app/config/config.toml \
  -e ALCHEMIST_DB_PATH=/app/data/alchemist.db \
  -e ALCHEMIST_CONFIG_MUTABLE=false \
  --name alchemist \
  alchemist
```

Access the web interface at `http://localhost:3000`.

## Configuration

Create a `config.toml` file in the working directory:

```toml
[transcode]
size_reduction_threshold = 0.3  # Require 30% size reduction
min_bpp_threshold = 0.1          # Minimum bits per pixel
min_file_size_mb = 50            # Skip files smaller than 50MB
concurrent_jobs = 2              # Number of parallel transcodes

[hardware]
allow_cpu_fallback = true        # Enable CPU encoding if no GPU
allow_cpu_encoding = true        # Allow software encoding
cpu_preset = "medium"            # CPU encoding speed: slow|medium|fast|faster

[scanner]
directories = [                  # Auto-scan directories
    "/media/movies",
    "/media/tvshows"
]
```

Runtime environment variables:

- `ALCHEMIST_CONFIG_PATH` config file path (default: `~/.openbitdo/config.toml` on Linux/macOS, `./config.toml` elsewhere)
- `ALCHEMIST_DB_PATH` SQLite database path (default: `~/.openbitdo/alchemist.db` on Linux/macOS, `./alchemist.db` elsewhere)
- `ALCHEMIST_CONFIG_MUTABLE` allow runtime config writes (`true`/`false`, default: `true`)

Most operational settings are managed from the web UI after first boot:

- `Transcode`: codec target, concurrency, thresholds, HDR handling
- `Files`: output suffix/extension, optional `output_root`, replace strategy, source deletion
- `Watch Folders`: add/remove canonicalized paths and choose recursive or top-level watching
- `Jobs`: cancel active work, restart terminal jobs, delete terminal history, and adjust job priority

When `output_root` is set, Alchemist mirrors the source-relative directory structure under that root. If it cannot determine a matching source root, it falls back to sibling output behavior.

## Supported Platforms

- **Linux**: x86_64 (Docker & Binary)
- **Windows**: x86_64
- **macOS**: x86_64 (Experimental)

## Usage

### CLI Mode

```bash
# Scan and transcode specific directories
alchemist --cli --dir /path/to/videos --dir /another/path

# Dry run (analyze only, don't transcode)
alchemist --cli --dry-run --dir /path/to/videos
```

### Server Mode

```bash
# Start web server on default port (3000)
alchemist
```

### Reset Auth

```bash
# Clear users/sessions and re-run setup
alchemist --reset-auth
```

## License

This project is licensed under the GPLv3 License - see the [LICENSE](LICENSE) file for details.
