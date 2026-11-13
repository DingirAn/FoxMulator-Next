#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
OUT=${1:-"$ROOT/dist/foxmulator-v${VERSION}-linux-x86_64"}
ARCHIVE="${OUT}.tar.gz"

cd "$ROOT"
cargo build --release -p foxmulator-desktop --features gui,audio,gamepad
mkdir -p "$OUT"
TARGET_DIR=$(cargo metadata --no-deps --format-version 1 \
  | python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
cp "$TARGET_DIR/release/foxmulator" "$OUT/foxmulator"
cp LICENSE README.md "$OUT/"
sha256sum "$OUT/foxmulator" "$OUT/LICENSE" "$OUT/README.md" > "$OUT/SHA256SUMS"

# Normalize archive metadata so identical source/build inputs produce the same
# distribution archive. ROMs and save files are deliberately never copied.
tar --sort=name \
    --mtime='UTC 1970-01-01' \
    --owner=0 --group=0 --numeric-owner \
    -czf "$ARCHIVE" \
    -C "$(dirname "$OUT")" "$(basename "$OUT")"

printf 'Release bundle: %s\n' "$OUT"
cat "$OUT/SHA256SUMS"
printf 'Release archive: %s\n' "$ARCHIVE"
sha256sum "$ARCHIVE"
