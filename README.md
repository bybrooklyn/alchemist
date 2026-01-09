# Alchemist

> Intelligent video transcoding automation with hardware acceleration and CPU fallback

Alchemist is a Rust-based video transcoding system that automatically converts your media library to efficient AV1 format using hardware acceleration (GPU) or software encoding (CPU fallback).

## Features

- **Hardware Acceleration**: Supports NVIDIA (NVENC), Intel (QSV), AMD (VAAPI) (macOS VideoToolbox is experimental/unsupported)
- **CPU Fallback**: Automatic software encoding with libsvtav1 when GPU is unavailable
- **Intelligent Analysis**: Only transcodes files that will benefit from AV1 encoding
- **Web Dashboard**: Real-time monitoring and control via Axum/Askama/HTMX-based UI
- **Single Binary**: All assets and templates are embedded into the binary for easy deployment
- **Background Processing**: Queue-based system with concurrent job support
- **Performance Metrics**: Detailed logging and statistics for each transcode job

## Quick Start

### Prerequisites

- **Rust**: 1.75+
- **FFmpeg**: With hardware acceleration support and libsvtav1
- **FFprobe**: For media analysis

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/alchemist.git
cd alchemist

# Build the project
cargo build --release

# Run the server
./target/release/alchemist --server
```

### Docker Deployment

```bash
# Build the Docker image
docker build -t alchemist .

# Run the container
docker run -d \
  -p 3000:3000 \
  -v /path/to/media:/media \
  -v /path/to/output:/output \
  --name alchemist \
  alchemist
```

Access the web interface at `http://localhost:3000`

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

## Supported Platforms

- **Linux**: x86_64, aarch64 (Docker & Binary)
- **Windows**: x86_64
- **macOS**: Currently unsupported / Experimental

## Usage

### CLI Mode

```bash
# Scan and transcode specific directories
alchemist /path/to/videos /another/path

# Dry run (analyze only, don't transcode)
alchemist --dry-run /path/to/videos
```

### Server Mode

```bash
# Start web server on default port (3000)
alchemist --server
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
