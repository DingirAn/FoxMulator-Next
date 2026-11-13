# FoxMulator minimal CGB BIOS (sketch)

This directory is an isolated RGBDS/RGBASM prototype. It deliberately has no
DMG compatibility path: FoxMulator selects its DMG and CGB BIOS separately.

The CGB BIOS keeps the essential hardware setup from SameBoy, but loads a
normal unique-tile stream and its generated 48-entry tilemap. It therefore does
not depend on SameBoy's fixed logo layout, repeated tiles, or PB12 compression.

The boot flow explicitly includes:

- CGB VRAM-bank and LCD initialization.
- Clearing both VRAM banks and OAM.
- Basic audio power-up (`NR52`, `NR11`, `NR12`, `NR51`, `NR50`).
- SameBoy-compatible two-note CGB boot chime on channel 1 (`NR13`/`NR14`).
- A 60-step, two-frame-per-step logo hold/fade, leaving the finished logo
  visible for roughly two seconds before post-boot cleanup.
- CGB background palette initialization and blue transition animation.
- VBlank-synchronized logo animation.
- `FF50` boot-ROM unmapping followed by the cartridge entry point.

## Build

From the repository root:

```sh
./bootrom-foxmulator/build.sh
```

The output is `bootrom-foxmulator/output/cgb_boot.bin`, and must be exactly
2304 bytes. The editable input is `assets/logo.png`, constrained to 128×24 and
at most four colors. The first visual validation exposed and fixed two prototype
issues: tile 0 is reserved as a blank background tile, and the local GTK viewer
uses the correct Cairo ARGB32 channel order. The captured frame now shows the
complete 128×24 logo on a clean background.

The intro reveals one generated tile per frame during VBlank. It is
intentionally independent of SameBoy's fixed repeated-tile animation, so
arbitrary logos keep their own generated tilemap.

## Visual tester

The folder includes an isolated GTK/SameBoy-Core tester:

```sh
./bootrom-foxmulator/preview-bootrom.sh cgb
```

Its staged Core, library, test cartridge, viewer, and captures remain under
`bootrom-foxmulator/build/boot-preview/`. The first run compiles the tester;
the window remains open until `Q`, Escape, or its close button is used. The
viewer sources and preview-ROM generator are copied into `tools/`, so the tester
does not depend on files from `bootrom-branding/`.

## CGB state audit

`tools/boot_state_dump.c` compares a boot ROM against the same cartridge and
records bounded state immediately before and after `FF50`, including CPU
registers, CGB I/O, VRAM/WRAM banks, and both palette RAMs. With Pokémon Gold
and 30,000,000 post-`FF50` ticks, the custom and SameBoy boot ROMs produced
identical 160×144 frames (`0` differing pixels). The custom BIOS restores the
white post-boot BG palettes and the CGB final register state after its custom
animation.
