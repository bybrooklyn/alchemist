# Alchemist

Alchemist is a next-generation "smart" media transcoding engine designed for extreme efficiency and reliability. Unlike traditional transcoders that blindly process every file, Alchemist utilizes a multi-stage analysis gate to decide whether a file actually *needs* to be transcoded, prioritizing file size reduction and quality retention.

## Key Features

- **Smart Analysis**: Preflight checks for codec, bitrate, and resolution to skip unnecessary work.
- **Hardware First**: Primary support for Intel Arc (AV1 QSV), NVIDIA (NVENC), and Apple (VideoToolbox).
- **Deterministic state**: A robust job state machine with persistent decision tracking ("Why did we skip this file?").
- **Real-Time Observation**: SSE-based live updates and progress tracking.
- **Fail-Loudly Policy**: Strict hardware enforcement to prevent poor-performance CPU fallbacks unless explicitly allowed.
- **Rust-Powered core**: Built for performance and reliability.

## Getting Started

### Prerequisites

- **FFmpeg**: Must be available in your PATH with appropriate hardware acceleration drivers (QSV, NVENC, or VAAPI).
- **SQLite**: Used for persistent job tracking.

### Running

To scan a directory and starting transcoding:
```bash
cargo run -- /path/to/media
```

To run as a web server:
```bash
cargo run -- --server
```

## Configuration

Alchemist looks for a `config.toml` in the working directory.

```toml
[transcode]
size_reduction_threshold = 0.3    # Fail if <30% reduction
concurrent_jobs = 1

[hardware]
allow_cpu_fallback = false        # Fail loudly if GPU is missing
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
