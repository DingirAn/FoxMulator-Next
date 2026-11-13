#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="$(cd "$ROOT_DIR" && cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
BIN="${1:-$TARGET_DIR/release/foxmulator}"
[[ -x "$BIN" ]] || { echo "Build the release binary first: $BIN" >&2; exit 1; }
ldd "$BIN"
