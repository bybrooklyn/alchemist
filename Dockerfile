# Using rustlang/rust:nightly for edition2024 support
FROM rustlang/rust:nightly AS chef
WORKDIR /app
RUN cargo install cargo-chef
RUN rustup target add wasm32-unknown-unknown

# Install cargo-binstall, then use it to get cargo-leptos (faster than compiling)
RUN curl -L https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-musl.tgz | tar -xz && \
    mv cargo-binstall /usr/local/bin/
RUN cargo binstall -y cargo-leptos

# Install tailwind standalone
RUN curl -sLO https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-x64 && \
    chmod +x tailwindcss-linux-x64 && \
    mv tailwindcss-linux-x64 /usr/local/bin/tailwindcss

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: Caching
FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Stage 3: Build Application
COPY . .
RUN cargo leptos build --release

# Stage 4: Runtime
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies: FFmpeg and HW drivers
# non-free and non-free-firmware are required for intel-media-va-driver-non-free
RUN apt-get update && apt-get install -y debian-archive-keyring && \
    echo "deb http://deb.debian.org/debian bookworm main contrib non-free non-free-firmware" > /etc/apt/sources.list && \
    echo "deb http://deb.debian.org/debian-security bookworm-security main contrib non-free non-free-firmware" >> /etc/apt/sources.list && \
    echo "deb http://deb.debian.org/debian bookworm-updates main contrib non-free non-free-firmware" >> /etc/apt/sources.list && \
    apt-get update && apt-get install -y \
    ffmpeg \
    intel-media-va-driver-non-free \
    libva-drm2 \
    libva2 \
    i965-va-driver \
    va-driver-all \
    libsqlite3-0 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary and assets
# cargo-leptos usually places the binary in target/server/release/
COPY --from=builder /app/target/server/release/alchemist /usr/local/bin/alchemist
COPY --from=builder /app/target/site /app/site

# Set environment variables
ENV LIBVA_DRIVER_NAME=iHD
ENV LEPTOS_SITE_ROOT=/app/site

# Entrypoint
ENTRYPOINT ["alchemist"]
CMD ["--server"]
