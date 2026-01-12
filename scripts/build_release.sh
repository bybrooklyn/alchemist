#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found; install Rust first." >&2
  exit 1
fi

if ! command -v bun >/dev/null 2>&1; then
  echo "bun not found; install Bun first." >&2
  exit 1
fi

if ! command -v zig >/dev/null 2>&1; then
  echo "zig not found; install Zig to cross-compile from macOS." >&2
  exit 1
fi

if ! command -v cargo-zigbuild >/dev/null 2>&1; then
  echo "cargo-zigbuild not found; install with 'cargo install cargo-zigbuild'." >&2
  exit 1
fi

if ! command -v cargo-xwin >/dev/null 2>&1; then
  echo "cargo-xwin not found; install with 'cargo install cargo-xwin' for Windows MSVC builds." >&2
  exit 1
fi

echo "Building web frontend..."
if [ ! -d "$ROOT_DIR/web/node_modules" ]; then
  (cd "$ROOT_DIR/web" && bun install)
fi
(cd "$ROOT_DIR/web" && bun run build)

TARGETS=(
  "aarch64-apple-darwin"
  "x86_64-unknown-linux-gnu"
  "x86_64-pc-windows-msvc"
)

HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"

build_target() {
  local target="$1"
  if [ "$HOST_OS" = "Darwin" ] && [ "$target" = "aarch64-apple-darwin" ] && [ "$HOST_ARCH" = "arm64" ]; then
    cargo build --release --target "$target"
  elif [[ "$target" == *"-pc-windows-msvc" ]]; then
    cargo xwin build --release --target "$target"
  else
    cargo zigbuild --release --target "$target"
  fi
}

echo "Building release binaries..."
for target in "${TARGETS[@]}"; do
  echo "- $target"
  rustup target add "$target" >/dev/null 2>&1 || true
  build_target "$target"
done

echo "Done. Artifacts are in target/<triple>/release/"
