#!/usr/bin/env python3
"""Packs the old project's font sheet into the 1-bit file the kernel includes.

Only needs running if the source image changes. Needs ImageMagick.
"""
import subprocess
from pathlib import Path

NEXT = Path(__file__).resolve().parent.parent
SHEET = NEXT.parent / "kernel/image/character-sheet-micro.png"
OUTPUT = NEXT / "kernel/src/font-16x30.bin"

SHEET_WIDTH, SHEET_HEIGHT = 256, 240
GLYPH_WIDTH, GLYPH_HEIGHT = 16, 30
COLUMNS, ROWS = 16, 8

alpha = subprocess.run(
    ["magick", str(SHEET), "-alpha", "extract", "gray:-"], capture_output=True, check=True
).stdout
assert len(alpha) == SHEET_WIDTH * SHEET_HEIGHT, len(alpha)

# Each glyph is 30 rows of 16 bits, most significant bit leftmost, in ASCII order.
packed = bytearray()
for code in range(COLUMNS * ROWS):
    left = (code % COLUMNS) * GLYPH_WIDTH
    top = (code // COLUMNS) * GLYPH_HEIGHT
    for y in range(GLYPH_HEIGHT):
        bits = 0
        for x in range(GLYPH_WIDTH):
            bits = bits << 1 | (alpha[(top + y) * SHEET_WIDTH + left + x] != 0)
        packed += bits.to_bytes(2, "big")

OUTPUT.write_bytes(packed)
print(f"wrote {OUTPUT}: {len(packed)} bytes")
