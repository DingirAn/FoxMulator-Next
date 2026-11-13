#!/usr/bin/env bash
set -euo pipefail

ARCHIVE=${1:?usage: scripts/verify-release.sh <release.tar.gz>}
[[ -f "$ARCHIVE" ]] || { echo "archive not found: $ARCHIVE" >&2; exit 2; }

ROOT_ENTRY=$(tar -tzf "$ARCHIVE" | sed -n '1p')
ROOT_ENTRY=${ROOT_ENTRY%/}
[[ -n "$ROOT_ENTRY" ]] || { echo "archive is empty" >&2; exit 1; }

tmp=$(mktemp -d "${TMPDIR:-/tmp}/foxmulator-verify.XXXXXX")
trap 'rm -rf "$tmp"' EXIT

entries=$(tar -tzf "$ARCHIVE")
while IFS= read -r entry; do
    case "$entry" in
        "$ROOT_ENTRY"/|"$ROOT_ENTRY"/foxmulator|"$ROOT_ENTRY"/LICENSE|\
        "$ROOT_ENTRY"/README.md|"$ROOT_ENTRY"/SHA256SUMS) ;;
        *) echo "unexpected archive entry: $entry" >&2; exit 1 ;;
    esac
done <<< "$entries"

tar -xzf "$ARCHIVE" -C "$tmp" --no-same-owner
bundle="$tmp/$ROOT_ENTRY"
[[ -x "$bundle/foxmulator" ]] || { echo "binary is not executable" >&2; exit 1; }
(cd "$bundle" && sha256sum -c SHA256SUMS)

printf 'release_archive=%s\n' "$ARCHIVE"
printf 'release_root=%s\n' "$ROOT_ENTRY"
printf 'archive_sha256='; sha256sum "$ARCHIVE" | cut -d' ' -f1
echo 'release_verification=PASS'
