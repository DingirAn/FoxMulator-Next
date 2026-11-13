#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="$(cd "$ROOT_DIR" && cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
APPDIR="${1:-$TARGET_DIR/appimage/foxmulator-next/AppDir}"

"$ROOT_DIR/packaging/generate-icons.sh"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor"
install -Dm755 "$TARGET_DIR/release/foxmulator" "$APPDIR/usr/bin/foxmulator"
install -Dm644 "$ROOT_DIR/packaging/foxmulator-next.desktop" "$APPDIR/usr/share/applications/foxmulator-next.desktop"
install -Dm644 "$ROOT_DIR/resources/foxmulator.png" "$APPDIR/usr/share/foxmulator-next/resources/foxmulator.png"
install -Dm644 "$ROOT_DIR/LICENSE" \
    "$APPDIR/usr/share/licenses/foxmulator-next/COPYING"
install -Dm644 "$ROOT_DIR/THIRD-PARTY-NOTICES.md" \
    "$APPDIR/usr/share/licenses/foxmulator-next/THIRD-PARTY-NOTICES.md"
install -Dm644 "$ROOT_DIR/SOURCE-CODE-CORRESPONDING.md" \
    "$APPDIR/usr/share/licenses/foxmulator-next/SOURCE-CODE-CORRESPONDING.md"
install -Dm644 "$ROOT_DIR/foxmulator-core/resources/bootroms/SAMEBOY-LICENSE.txt" \
    "$APPDIR/usr/share/licenses/foxmulator-next/SAMEBOY-LICENSE.txt"
install -Dm644 "$ROOT_DIR/foxmulator-core/resources/bootroms/HARDWARE-INC-CC0.txt" \
    "$APPDIR/usr/share/licenses/foxmulator-next/HARDWARE-INC-CC0.txt"
for license in "$ROOT_DIR"/docs/licenses/slint/*; do
    install -Dm644 "$license" \
        "$APPDIR/usr/share/licenses/foxmulator-next/slint/$(basename "$license")"
done
for size in 16 24 32 48 64 128 256 512; do
    install -Dm644 "$ROOT_DIR/resources/icons/hicolor/${size}x${size}/apps/foxmulator-next.png" \
        "$APPDIR/usr/share/icons/hicolor/${size}x${size}/apps/foxmulator-next.png"
done
echo "Assembled AppDir: $APPDIR"
