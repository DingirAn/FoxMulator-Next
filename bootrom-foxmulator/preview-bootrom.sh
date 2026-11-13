#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

mode="${1:-cgb}"
if [[ "$mode" != cgb ]]; then
    echo "Uso: $0 cgb" >&2
    echo "Esta carpeta contiene deliberadamente solo el tester CGB." >&2
    exit 2
fi

sameboy_src="${SAMEBOY_SRC:-../referencias/SameBoy}"
tool_dir="$PWD/tools"
build_dir="$PWD/build/boot-preview"
stage="$build_dir/sameboy-src"
lib="$build_dir/lib/libsameboy.a"

./build.sh
python3 "$tool_dir/make_preview_rom.py" cgb "$build_dir/preview-cgb.gb"

if [[ ! -f "$lib" ]]; then
    command -v make >/dev/null || { echo "Falta make" >&2; exit 1; }
    command -v cc >/dev/null || { echo "Falta cc" >&2; exit 1; }
    mkdir -p "$stage"
    tar --exclude=.git --exclude=build -cf - -C "$sameboy_src" . | tar -xf - -C "$stage"
    make -B -C "$stage" lib CONF=release DISABLE_TIMEKEEPING=1 DISABLE_REWIND=1 DISABLE_DEBUGGER=1 \
      BIN="$build_dir/bin" OBJ="$build_dir/obj" INC="$build_dir/include" \
      LIBDIR="$build_dir/lib" PKGCONF_DIR="$build_dir/pkgconfig"
fi

if [[ ! -x "$build_dir/boot_preview" || "$tool_dir/boot_preview.c" -nt "$build_dir/boot_preview" ]]; then
    pkg-config --exists gtk+-3.0 || { echo "Falta GTK+ 3 de desarrollo" >&2; exit 1; }
    cc -O2 -Wall -Wextra -Werror -Wno-multichar -DGB_INTERNAL \
      -DGB_DISABLE_TIMEKEEPING -DGB_DISABLE_REWIND -DGB_DISABLE_DEBUGGER -DGB_DISABLE_CHEAT_SEARCH \
      -I"$stage/Core" \
      $(pkg-config --cflags gtk+-3.0) "$tool_dir/boot_preview.c" "$lib" \
      $(pkg-config --libs gtk+-3.0) -lm -ldl -o "$build_dir/boot_preview"
fi

echo "BIOS CGB: $PWD/output/cgb_boot.bin"
echo "Pausa con Espacio; S captura cuando está pausado; Q/Escape sale."
exec "$build_dir/boot_preview" cgb output/cgb_boot.bin "$build_dir/preview-cgb.gb"
