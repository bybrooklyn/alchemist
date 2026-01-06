# Stage 1: Preparation
FROM lukemathwalker/cargo-chef:latest-rust-1.81 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: Caching
FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching layer
RUN cargo chef cook --release --recipe-path recipe.json

# Stage 3: Build Application
COPY . .
RUN cargo build --release

# Stage 4: Runtime
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies: FFmpeg and HW drivers
# non-free is required for intel-media-va-driver-non-free
RUN apt-get update && apt-get install -y \
    ffmpeg \
    intel-media-va-driver-non-free \
    libva-drm2 \
    libva2 \
    i965-va-driver \
    va-driver-all \
    libsqlite3-0 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/alchemist /usr/local/bin/alchemist

# Set environment variables for hardware accelaration
ENV LIBVA_DRIVER_NAME=iHD

# Entrypoint
ENTRYPOINT ["alchemist"]
CMD ["--server"]
