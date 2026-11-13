#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ICON_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"
APP_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

[[ -f "$ROOT_DIR/resources/foxmulator.png" ]] || { echo "Missing icon" >&2; exit 1; }
install -Dm644 "$ROOT_DIR/packaging/foxmulator-next.desktop" "$APP_ROOT/foxmulator-next.desktop"
for size in 16 24 32 48 64 128 256 512; do
    install -Dm644 "$ROOT_DIR/resources/icons/hicolor/${size}x${size}/apps/foxmulator-next.png" \
        "$ICON_ROOT/${size}x${size}/apps/foxmulator-next.png"
done
echo "Installed launcher: $APP_ROOT/foxmulator-next.desktop"
echo "The executable must be available as 'foxmulator' in PATH."
