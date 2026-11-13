#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="$(cd "$ROOT_DIR" && cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
PACKAGING_DIR="$TARGET_DIR/appimage/foxmulator-next"
APPDIR="$PACKAGING_DIR/AppDir"
OUTPUT="${FOX_APPIMAGE_OUTPUT:-$ROOT_DIR/FoxMulator-Next-x86_64.AppImage}"

# Cargo's configured target directory may be shared by more than one project.
# Keep this package's staging area project-specific and refuse to remove an
# unowned directory. The staging area is retained on failure for diagnostics
# and removed only after appimagetool succeeds.
OWNER_MARKER="$PACKAGING_DIR/.foxmulator-next-owned"
cleanup_on_success=0
cleanup() {
    if (( cleanup_on_success )); then
        rm -rf -- "$PACKAGING_DIR"
    fi
}
trap cleanup EXIT

for tool in cargo convert ldd; do
    if ! command -v "$tool" >/dev/null; then
        echo "Missing required tool: $tool" >&2
        echo "Install it separately; this script never installs packages." >&2
        exit 2
    fi
done

cargo build --release -p foxmulator-desktop --features gui,audio,gamepad
if [[ -e "$PACKAGING_DIR" && ! -f "$OWNER_MARKER" ]]; then
    echo "Refusing to remove unowned packaging directory: $PACKAGING_DIR" >&2
    exit 2
fi
mkdir -p "$PACKAGING_DIR"
touch "$OWNER_MARKER"
rm -rf -- "$APPDIR"
"$ROOT_DIR/packaging/assemble-appdir.sh" "$APPDIR"

for tool in linuxdeploy appimagetool; do
    if ! command -v "$tool" >/dev/null; then
        echo "AppDir is ready, but packaging tool is missing: $tool" >&2
        echo "Install linuxdeploy and appimagetool separately, then rerun this script." >&2
        exit 2
    fi
done

extra_libraries=()
for library in libfontconfig.so.1 libfreetype.so.6 libexpat.so.1 libz.so.1; do
    library_path="$(ldconfig -p | awk -v name="$library" '$1 == name { print $NF; exit }')"
    if [[ -z "$library_path" ]]; then
        echo "Missing required runtime library: $library" >&2
        exit 2
    fi
    extra_libraries+=(--library "$library_path")
done

linuxdeploy --appdir "$APPDIR" \
    "${extra_libraries[@]}" \
    --desktop-file "$APPDIR/usr/share/applications/foxmulator-next.desktop" \
    --icon-file "$APPDIR/usr/share/icons/hicolor/256x256/apps/foxmulator-next.png"

# linuxdeploy normally chooses its 64px icon for the AppImage root links.
# Nemo uses .DirIcon for the AppImage file itself, so keep the high-resolution
# 256px source as the primary AppImage icon while retaining all Hicolor sizes.
ln -sfn usr/share/icons/hicolor/256x256/apps/foxmulator-next.png "$APPDIR/.DirIcon"
ln -sfn usr/share/icons/hicolor/256x256/apps/foxmulator-next.png "$APPDIR/foxmulator-next.png"

appimagetool "$APPDIR" "$OUTPUT"
cleanup_on_success=1
echo "Created $OUTPUT"
