# Alchemist

> Intelligent video transcoding automation with hardware acceleration and CPU fallback

Alchemist is a Rust-based video transcoding system that automatically converts your media library to efficient AV1 format using hardware acceleration (GPU) or software encoding (CPU fallback).

## Features

- ** Hardware Acceleration**: Supports NVIDIA (NVENC), Intel (QSV), AMD (VAAPI), and Apple (VideoToolbox)
- ** CPU Fallback**: Automatic software encoding with libsvtav1 when GPU is unavailable
- ** Intelligent Analysis**: Only transcodes files that will benefit from AV1 encoding
- ** Web Dashboard**: Real-time monitoring and control via Leptos-based UI
- ** Background Processing**: Queue-based system with concurrent job support
- ** Performance Metrics**: Detailed logging and statistics for each transcode job

## Quick Start

### Prerequisites

- **Rust**: 1.75+ (nightly for WASM support)
- **FFmpeg**: With hardware acceleration support and libsvtav1
- **FFprobe**: For media analysis
- **Docker** (optional): For containerized deployment

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

## Architecture

### Components

```
┌─────────────────────────────────────────────────────────────┐
│                      Web Interface (Leptos)                  │
│                    http://localhost:3000                     │
└───────────────────────────┬─────────────────────────────────┘
                            │
┌───────────────────────────┴─────────────────────────────────┐
│                      Axum HTTP Server                       │
│                    (API + Static Assets)                    │
└───────────────────────────┬─────────────────────────────────┘
                            │
            ┌───────────────┼───────────────┐
            │               │               │
┌───────────▼────────┐ ┌────▼───────┐ ┌─────▼──────┐
│     Scanner        │ │  Analyzer  │ │  Database  │
│  (Media Discovery) │ │  (FFprobe) │ │  (SQLite)  │
└────────────────────┘ └────────────┘ └─────┬──────┘
                                            │
            ┌───────────────────────────────┘
            │
┌───────────▼────────────────────────────────────────┐
│              Processor (Job Queue)                  │
│          • Concurrent job execution                 │
│          • State management                         │
└───────────┬────────────────────────────────────────┘
            │
┌───────────▼────────────────────────────────────────┐
│           Orchestrator (FFmpeg Wrapper)             │
│                                                     │
│  GPU Mode:           CPU Mode:                     │
│  • av1_qsv  (Intel)  • libsvtav1 (software)        │
│  • av1_nvenc (NVIDIA)                              │
│  • av1_vaapi (AMD)                                 │
└─────────────────────────────────────────────────────┘
```

### Database Location

**Current Behavior:**
- The SQLite database (`alchemist.db`) is created in the **current working directory** when the application starts
- This is intentional for flexibility during development and deployment

**Future Configuration:**
- A configurable database path will be added via:
  - Environment variable: `ALCHEMIST_DB_PATH`
  - Configuration file: `config.toml`
  - Docker mount point: `/app/data/alchemist.db`

**Docker Recommendation:**
For production use with Docker, mount a persistent volume:
```bash
docker run -v /host/data:/app/data alchemist
```

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
# preferred_vendor = "intel"     # Force specific GPU vendor

[scanner]
directories = [                  # Auto-scan directories
    "/media/movies",
    "/media/tvshows"
]
```

## Usage

### CLI Mode

```bash
# Scan and transcode specific directories
alchemist /path/to/videos /another/path

# Dry run (analyze only, don't transcode)
alchemist --dry-run /path/to/videos

# Specify output directory
alchemist --output-dir /path/to/output /path/to/input
```

### Server Mode

```bash
# Start web server on default port (3000)
alchemist --server

# Access web interface
open http://localhost:3000
```

## Hardware Acceleration

### Supported Platforms

| Vendor | Encoder | Quality | Speed | Linux | macOS | Windows |
|--------|---------|---------|-------|-------|-------|---------|
| **Intel QSV** | `av1_qsv` | Good | Fast | ✅ | ✅ | ✅ |
| **NVIDIA NVENC** | `av1_nvenc` | Excellent | Very Fast | ✅ | ❌ | ✅ |
| **AMD VAAPI** | `av1_vaapi` | Good | Fast | ✅ | ❌ | ❌ |
| **Apple VideoToolbox** | `av1_videotoolbox` | Excellent | Fast | ❌ | ✅ | ❌ |
| **CPU (libsvtav1)** | `libsvtav1` | Excellent | Slow | ✅ | ✅ | ✅ |

### CPU Encoding Performance

CPU encoding is **10-50x slower** than GPU acceleration, depending on resolution:

| Resolution | GPU Time | CPU Time (Preset 8) | Recommended |
|------------|----------|---------------------|-------------|
| 1080p | 2-5 min | 20-60 min | ✅ Acceptable |
| 4K | 5-15 min | 1-4 hours | ⚠️ Slow |
| 8K | 15-45 min | 4-12 hours | ❌ Very Slow |

**CPU Preset Guide:**
- `slow` (0-4): Best quality, extremely slow
- `medium` (5-8): Balanced (recommended for CPU)
- `fast` (9-13): Fastest, lower quality

## Development

### Project Structure

```
alchemist/
├── src/
│   ├── main.rs           # Entry point and CLI
│   ├── lib.rs            # Library exports
│   ├── server.rs         # Axum web server
│   ├── app.rs            # Leptos UI components
│   ├── hardware.rs       # GPU/CPU detection
│   ├── config.rs         # Configuration management
│   ├── scanner.rs        # Media file discovery
│   ├── analyzer.rs       # FFprobe wrapper
│   ├── processor.rs      # Job queue processor
│   ├── orchestrator.rs   # FFmpeg orchestration
│   ├── db.rs             # SQLite database
│   └── error.rs          # Error types
├── style/                # Tailwind CSS
├── public/               # Static assets
├── Cargo.toml            # Rust dependencies
├── Dockerfile            # Container definition
└── config.toml           # Configuration (optional)
```

### Building for Development

```bash
# Install cargo-leptos for dev server with hot reload
cargo install cargo-leptos

# Run development server
cargo leptos watch

# Run tests
cargo test

# Check code
cargo check
```

## Troubleshooting

### GPU Not Detected

1. **Check FFmpeg support**:
   ```bash
   ffmpeg -encoders | grep av1
   ```

2. **Verify device nodes** (Linux):
   ```bash
   ls -la /dev/dri/render*
   ```

3. **Check NVIDIA drivers**:
   ```bash
   nvidia-smi
   ```

### CPU Encoding Too Slow

1. **Adjust preset** in `config.toml`:
   ```toml
   [hardware]
   cpu_preset = "faster"  # Fastest CPU encoding
   ```

2. **Reduce concurrent jobs**:
   ```toml
   [transcode]
   concurrent_jobs = 1
   ```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
