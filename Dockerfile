# Stage 1: Build Frontend with Bun
FROM oven/bun:1 AS frontend-builder
WORKDIR /app
COPY web/package.json web/bun.lock* ./
RUN bun install --frozen-lockfile
COPY web/ .
RUN bun run build

# Stage 2: Rust Chef Planner
FROM rust:1-slim-bookworm AS chef
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
RUN mkdir -p /app/config /app/data

# Install runtime dependencies
RUN apt-get update && \
    sed -i 's/main/main contrib non-free non-free-firmware/g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && apt-get install -y --no-install-recommends \
    wget \
    xz-utils \
    libva-drm2 \
    libva2 \
    va-driver-all \
    libsqlite3-0 \
    ca-certificates \
    gosu \
    && if [ "$(dpkg --print-architecture)" = "amd64" ]; then \
    apt-get install -y --no-install-recommends \
    intel-media-va-driver-non-free \
    i965-va-driver || true; \
    fi \
    && rm -rf /var/lib/apt/lists/*

# Download stable FFmpeg static build (v7.1)
RUN set -e; \
    ARCH=$(dpkg --print-architecture); \
    if [ "$ARCH" = "amd64" ]; then \
      ARCHIVE="ffmpeg-release-amd64-static.tar.xz"; \
      URL="https://johnvansickle.com/ffmpeg/releases/${ARCHIVE}"; \
      SHA256="abda8d77ce8309141f83ab8edf0596834087c52467f6badf376a6a2a4c87cf67"; \
    elif [ "$ARCH" = "arm64" ]; then \
      ARCHIVE="ffmpeg-release-arm64-static.tar.xz"; \
      URL="https://johnvansickle.com/ffmpeg/releases/${ARCHIVE}"; \
      SHA256="f4149bb2b0784e30e99bdda85471c9b5930d3402014e934a5098b41d0f7201b1"; \
    else \
      echo "Unsupported architecture: $ARCH" >&2; \
      exit 1; \
    fi; \
    wget -O "$ARCHIVE" "$URL"; \
    echo "${SHA256}  ${ARCHIVE}" | sha256sum -c -; \
    tar xf "$ARCHIVE"; \
    mv ffmpeg-*-static/ffmpeg /usr/local/bin/; \
    mv ffmpeg-*-static/ffprobe /usr/local/bin/; \
    rm -rf "$ARCHIVE" ffmpeg-*-static

COPY --from=builder /app/target/release/alchemist /usr/local/bin/alchemist

# Set environment variables
# VA-API driver auto-detection: do NOT hardcode LIBVA_DRIVER_NAME here.
# Users can override via: docker run -e LIBVA_DRIVER_NAME=iHD ...
# Common values: iHD (Intel ≥ Broadwell), i965 (older Intel), radeonsi (AMD)
ENV RUST_LOG=info
ENV ALCHEMIST_CONFIG_PATH=/app/config/config.toml
ENV ALCHEMIST_DB_PATH=/app/data/alchemist.db
COPY entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

EXPOSE 3000

ENTRYPOINT ["/app/entrypoint.sh"]
CMD ["alchemist"]
