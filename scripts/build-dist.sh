#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"

mkdir -p "$DIST_DIR"

"$ROOT_DIR/scripts/prepare-icons.sh"

if [ -x "$HOME/.cargo/bin/cargo" ]; then
  CARGO_BIN="$HOME/.cargo/bin/cargo"
elif command -v cargo >/dev/null 2>&1; then
  CARGO_BIN="$(command -v cargo)"
else
  echo "cargo not found. Install Rust toolchain first."
  exit 1
fi

CARGO_VERSION_RAW="$("$CARGO_BIN" --version | awk '{print $2}')"
MIN_CARGO_VERSION="1.85.0"
if [ "$(printf '%s\n' "$MIN_CARGO_VERSION" "$CARGO_VERSION_RAW" | sort -V | head -n1)" != "$MIN_CARGO_VERSION" ]; then
  echo "cargo $CARGO_VERSION_RAW is too old. Required >= $MIN_CARGO_VERSION."
  echo "Using cargo binary: $CARGO_BIN"
  exit 1
fi

"$CARGO_BIN" build --release --manifest-path "$ROOT_DIR/Cargo.toml"
cp "$ROOT_DIR/target/release/yandex-messenger" "$DIST_DIR/"

if command -v debuild >/dev/null 2>&1; then
  (
    cd "$ROOT_DIR"
    debuild -us -uc -b
  )
  cp "$ROOT_DIR"/../yandex-messenger-native_*_amd64.deb "$DIST_DIR/" 2>/dev/null || true
  cp "$ROOT_DIR"/../yandex-messenger-native_*_amd64.changes "$DIST_DIR/" 2>/dev/null || true
else
  echo "debuild not found, skipping .deb creation."
fi

echo "Distribution artifacts are in: $DIST_DIR"
