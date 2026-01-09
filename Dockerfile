# Stage 1: Build Frontend with Bun
FROM oven/bun:1 AS frontend-builder
WORKDIR /app
COPY web/package.json web/bun.lockb* ./
RUN bun install --frozen-lockfile
COPY web/ .
RUN bun run build

# Stage 2: Rust Chef Planner
FROM rustlang/rust:nightly AS chef
WORKDIR /app
RUN cargo install cargo-chef

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build Rust Backend
FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is cached!
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
# Copy built frontend assets so rust-embed can find them
COPY --from=frontend-builder /app/dist ./web/dist
RUN cargo build --release

# Stage 4: Runtime
FROM debian:testing-slim AS runtime
WORKDIR /app

# Enable non-free repositories and install packages
# Note: Intel VA drivers are x86-only, we install them conditionally
RUN apt-get update && \
    sed -i 's/main/main contrib non-free non-free-firmware/g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    libva-drm2 \
    libva2 \
    va-driver-all \
    libsqlite3-0 \
    ca-certificates \
    && if [ "$(dpkg --print-architecture)" = "amd64" ]; then \
    apt-get install -y --no-install-recommends \
    intel-media-va-driver-non-free \
    i965-va-driver || true; \
    fi \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/alchemist /usr/local/bin/alchemist

# Set environment variables
ENV LIBVA_DRIVER_NAME=iHD
ENV RUST_LOG=info
EXPOSE 3000

ENTRYPOINT ["alchemist"]
CMD ["--server"]
