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
FROM debian:trixie-slim AS runtime

LABEL org.opencontainers.image.source="https://github.com/bybrooklyn/alchemist" \
      org.opencontainers.image.description="Alchemist — self-hosted media transcoding pipeline" \
      org.opencontainers.image.licenses="AGPL-3.0-or-later"

WORKDIR /app
RUN mkdir -p /app/config /app/data

# Install runtime dependencies
RUN apt-get update && \
    sed -i 's/main/main contrib non-free non-free-firmware/g' /etc/apt/sources.list.d/debian.sources && \
    apt-get update && apt-get install -y --no-install-recommends \
    wget \
    libva-drm2 \
    libva2 \
    va-driver-all \
    libsqlite3-0 \
    ca-certificates \
    gosu \
    tini \
    && if [ "$(dpkg --print-architecture)" = "amd64" ]; then \
    apt-get install -y --no-install-recommends \
    intel-media-va-driver-non-free \
    i965-va-driver || true; \
    fi \
    && rm -rf /var/lib/apt/lists/*

# Install FFmpeg via jellyfin-ffmpeg.
# Generic static builds (e.g. johnvansickle) ship WITHOUT the hardware encoders
# (no --enable-vaapi/--enable-libvpl/--enable-nvenc), so VAAPI/QSV/NVENC probes fail
# with "Unknown encoder" and every GPU silently falls back to CPU. jellyfin-ffmpeg
# bundles the Intel (VAAPI + QSV via oneVPL), AMD (VAAPI), and NVIDIA (NVENC) encoders
# plus the matching runtime, so /dev/dri passthrough works. Pinned + checksum-verified.
RUN set -e; \
    JF_VERSION="7.1.4-3"; \
    ARCH=$(dpkg --print-architecture); \
    if [ "$ARCH" = "amd64" ]; then \
      DEB="jellyfin-ffmpeg7_${JF_VERSION}-trixie_amd64.deb"; \
      SHA256="8e30f9f7f3958c524bec8334540e4241145ad4300b328a08a55783c619187225"; \
    elif [ "$ARCH" = "arm64" ]; then \
      DEB="jellyfin-ffmpeg7_${JF_VERSION}-trixie_arm64.deb"; \
      SHA256="a8fa3ec7cf8fbaf06bb4fdb768d4dd7798277fe4c4b66884c777dc7d575877fb"; \
    else \
      echo "Unsupported architecture: $ARCH" >&2; \
      exit 1; \
    fi; \
    wget -O "$DEB" "https://github.com/jellyfin/jellyfin-ffmpeg/releases/download/v${JF_VERSION}/${DEB}"; \
    echo "${SHA256}  ${DEB}" | sha256sum -c -; \
    apt-get update; \
    apt-get install -y --no-install-recommends "./${DEB}"; \
    rm -f "$DEB"; \
    rm -rf /var/lib/apt/lists/*; \
    ln -sf /usr/lib/jellyfin-ffmpeg/ffmpeg /usr/local/bin/ffmpeg; \
    ln -sf /usr/lib/jellyfin-ffmpeg/ffprobe /usr/local/bin/ffprobe

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

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
    CMD wget -qO- "http://127.0.0.1:${ALCHEMIST_SERVER_PORT:-3000}/api/health" >/dev/null || exit 1

ENTRYPOINT ["tini", "--", "/app/entrypoint.sh"]
CMD ["alchemist"]
