#!/usr/bin/env python3
"""Reserve tile 0 as the blank CGB background tile."""
from pathlib import Path
import sys

path = Path(sys.argv[1])
data = bytearray(path.read_bytes())
if len(data) != 48:
    raise SystemExit(f"expected a 16x3 tilemap, got {len(data)} bytes")
if any(value >= 255 for value in data):
    raise SystemExit("tilemap cannot be shifted without overflowing an ID")
path.write_bytes(bytes(value + 1 for value in data))
