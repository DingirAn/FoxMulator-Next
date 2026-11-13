#!/usr/bin/env python3
"""Normalize an editable logo to an indexed PNG accepted by rgbgfx."""
from pathlib import Path
from PIL import Image

root = Path(__file__).resolve().parents[1]
source = Image.open(root / "assets" / "logo.png").convert("RGB")
if source.size != (128, 24):
    raise SystemExit(f"logo must be 128x24, got {source.size[0]}x{source.size[1]}")
colors = source.getcolors(maxcolors=5)
if not colors or len(colors) > 4:
    raise SystemExit("logo must use between 1 and 4 colors")

ordered = sorted((color for _, color in colors), key=lambda c: sum(c), reverse=True)
indexed = Image.new("P", source.size)
palette = [channel for color in ordered for channel in color]
palette.extend([0] * (768 - len(palette)))
indexed.putpalette(palette)
ids = {color: index for index, color in enumerate(ordered)}
indexed.putdata([ids[color] for color in source.getdata()])
indexed.save(root / "build" / "logo-indexed.png", format="PNG", optimize=False)
print(f"Logo PASS: 128x24, colors={len(ordered)}")
