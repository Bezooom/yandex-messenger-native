#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_ICON="$ROOT_DIR/icons/yandex-messenger.svg"
OUT_DIR="$ROOT_DIR/icons/hicolor"
FLAT_PNG="$ROOT_DIR/icons/yandex-messenger.png"
ASSETS_PNG="$ROOT_DIR/assets/yandex-messenger.png"
ASSETS_ICON="$ROOT_DIR/assets/icon.png"

mkdir -p "$OUT_DIR/scalable/apps"
cp "$SRC_ICON" "$OUT_DIR/scalable/apps/yandex-messenger.svg"

render() {
  local size="$1"
  local out="$2"
  if command -v rsvg-convert >/dev/null 2>&1; then
    rsvg-convert -w "$size" -h "$size" "$SRC_ICON" -o "$out"
  elif command -v convert >/dev/null 2>&1; then
    convert -background none -resize "${size}x${size}" "$SRC_ICON" "$out"
  else
    echo "Need rsvg-convert or ImageMagick convert" >&2
    exit 1
  fi
}

for size in 16 24 32 48 64 128 256 512; do
  mkdir -p "$OUT_DIR/${size}x${size}/apps"
  render "$size" "$OUT_DIR/${size}x${size}/apps/yandex-messenger.png"
done

render 256 "$FLAT_PNG"
render 256 "$ASSETS_PNG"
render 512 "$ASSETS_ICON"

echo "Icon set prepared in $OUT_DIR"
