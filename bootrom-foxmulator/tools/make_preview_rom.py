#!/usr/bin/env python3
"""Create a tiny original cartridge used only by the Boot ROM preview."""
from pathlib import Path
import sys

LOGO = bytes.fromhex(
    "ce ed 66 66 cc 0d 00 0b 03 73 00 83 00 0c 00 0d "
    "00 08 11 1f 88 89 00 0e dc cc 6e e6 dd dd d9 99 "
    "bb bb 67 63 6e 0e ec cc dd dc 99 9f bb b9 33 3e"
)

def main() -> None:
    mode = sys.argv[1] if len(sys.argv) > 1 else "cgb"
    if mode not in {"cgb", "dmg"}:
        raise SystemExit("usage: make_preview_rom.py [cgb|dmg] [output]")
    output = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("build/preview.gb")
    rom = bytearray([0] * 0x8000)
    rom[0x100:0x104] = b"\x00\xC3\x50\x01"
    # The CGB preview is meant to isolate the Boot ROM branding. A zeroed
    # cartridge logo keeps its separate header animation from being mistaken
    # for corruption above the FoxMulator logo. The DMG loader still receives
    # the standard compatibility logo.
    header_logo = bytes(48) if mode == "cgb" else LOGO
    if len(header_logo) != 48:
        raise SystemExit("invalid preview cartridge header logo")
    rom[0x104:0x134] = header_logo
    rom[0x134:0x144] = b"FOX PREVIEW\x00\x00\x00\x00\x00"
    rom[0x143] = 0x80 if mode == "cgb" else 0x00
    rom[0x147:0x14C] = b"\x00\x00\x00\x00\x00"
    rom[0x14B] = 0x33
    rom[0x150:0x153] = b"\x18\xFE\x00"  # JR $0150; NOP padding
    checksum = 0
    for value in rom[0x134:0x14D]:
        checksum = (checksum - value - 1) & 0xFF
    rom[0x14D] = checksum
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(rom)

if __name__ == "__main__":
    main()
