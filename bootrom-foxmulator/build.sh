#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

RGBDS_BIN="${RGBDS_BIN:-../referencias/rgbds}"
mkdir -p build output
python3 tools/prepare_logo.py

# The minimal BIOS deliberately uses the ordinary unique-tile order. Its
# generated tilemap is copied to VRAM, so no SameBoy-specific tile sequence is
# required and every 128x24, <=4-color image can use its own layout.
"$RGBDS_BIN/rgbgfx" -u -o build/logo.2bpp -t build/tilemap.bin -c embedded build/logo-indexed.png
python3 tools/offset_tilemap.py build/tilemap.bin
tile_bytes=$(wc -c < build/logo.2bpp)
if (( tile_bytes == 0 || tile_bytes % 16 != 0 || tile_bytes > 48 * 16 )); then
    echo "invalid tile stream size: $tile_bytes (expected 1..768 bytes)" >&2
    exit 1
fi
printf 'DEF LOGO_TILE_BYTES EQU %d\n' "$tile_bytes" > build/logo.inc
printf 'DEF LOGO_TILE_COUNT EQU %d\n' "$((tile_bytes / 16))" >> build/logo.inc

"$RGBDS_BIN/rgbasm" -I src -I build -o build/cgb_boot.o src/cgb_boot.asm
"$RGBDS_BIN/rgblink" -x -o output/cgb_boot.bin build/cgb_boot.o
size=$(wc -c < output/cgb_boot.bin)
if (( size != 2304 )); then
    echo "invalid CGB BIOS size: $size (expected 2304)" >&2
    exit 1
fi
printf 'CGB minimal BIOS: PASS size=%s tiles=%s\n' "$size" "$((tile_bytes / 16))"
