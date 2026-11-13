#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE="$ROOT_DIR/resources/foxmulator.png"
[[ -f "$SOURCE" ]] || { echo "Missing icon: $SOURCE" >&2; exit 1; }
for size in 16 24 32 48 64 128 256 512; do
    target="$ROOT_DIR/resources/icons/hicolor/${size}x${size}/apps/foxmulator-next.png"
    mkdir -p "$(dirname "$target")"
    convert "$SOURCE" -background none -filter Lanczos -resize "${size}x${size}" PNG32:"$target"
done
