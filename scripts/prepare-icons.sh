#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_ICON="$ROOT_DIR/icons/yandex-messenger.svg"
OUT_DIR="$ROOT_DIR/icons/hicolor"
FLAT_PNG="$ROOT_DIR/icons/yandex-messenger.png"

mkdir -p "$OUT_DIR/scalable/apps"
cp "$SRC_ICON" "$OUT_DIR/scalable/apps/yandex-messenger.svg"

if command -v rsvg-convert >/dev/null 2>&1; then
  for size in 16 24 32 48 64 128 256; do
    mkdir -p "$OUT_DIR/${size}x${size}/apps"
    rsvg-convert -w "$size" -h "$size" "$SRC_ICON" > "$OUT_DIR/${size}x${size}/apps/yandex-messenger.png"
  done
  # Keep a predictable flat PNG path for tooling/manual checks.
  rsvg-convert -w 256 -h 256 "$SRC_ICON" > "$FLAT_PNG"
fi

echo "Icon set prepared in $OUT_DIR"
