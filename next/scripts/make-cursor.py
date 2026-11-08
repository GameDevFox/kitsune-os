#!/usr/bin/env python3
"""Makes the cursor data the kernel includes from `kernel/src/cursor.png`.

That image is the cursor, edited by hand, so change it there and run this. It needs to stay
SIZE_PIXELS square, with every pixel fully opaque or fully transparent. Needs ImageMagick.
Prints the position of the tip, which `kernel/src/cursor.rs` must agree with.
"""
import importlib.util
import struct
import subprocess
import sys
from pathlib import Path

NEXT = Path(__file__).resolve().parent.parent
SOURCE = NEXT / "kernel/src/cursor.png"
OUTPUT = NEXT / "kernel/src/cursor.bin"
SIZE_PIXELS = 32
# The pixel it points with: the middle of the rounded corner, which mirrors around it.
TIP = (2, 2)
PIXEL_BYTES = 4
ALPHA_AT = 3
TRANSPARENT = 0
OPAQUE = 255
FULL_COLOR_INDEX_SIZE = 1

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("make_assets", NEXT / "scripts/make-assets.py")
make_assets = importlib.util.module_from_spec(spec)
spec.loader.exec_module(make_assets)


def colors_from(width, height, raw):
    """Returns a color or None per pixel, refusing what the kernel's format can't show."""
    if (width, height) != (SIZE_PIXELS, SIZE_PIXELS):
        raise SystemExit(f"the cursor must be {SIZE_PIXELS}x{SIZE_PIXELS}, not {width}x{height}")
    colors = []
    for at in range(0, len(raw), PIXEL_BYTES):
        alpha = raw[at + ALPHA_AT]
        if alpha not in (TRANSPARENT, OPAQUE):
            pixel = at // PIXEL_BYTES
            raise SystemExit(f"pixel ({pixel % width}, {pixel // width}) is partly transparent")
        colors.append(bytes(raw[at:at + ALPHA_AT]) if alpha == OPAQUE else None)
    return colors


def render():
    """Returns the width, the height, a color or None per pixel, and the tip's position."""
    width, height, raw = make_assets.load_rgba(SOURCE)
    return width, height, colors_from(width, height, raw), TIP


def main():
    width, height, colors, tip = render()
    palette = sorted({color for color in colors if color is not None})
    index_of = {color: number + 1 for number, color in enumerate(palette)}
    index_of[None] = 0
    symbols = [bytes([index_of[color]]) for color in colors]
    encoded = make_assets.encode_symbols(symbols)
    if make_assets.decode_symbols(encoded, FULL_COLOR_INDEX_SIZE) != b"".join(symbols):
        raise SystemExit("the cursor doesn't decode back to what was encoded")
    OUTPUT.write_bytes(struct.pack("<HHBH", width, height, FULL_COLOR_INDEX_SIZE, len(palette)) + b"".join(palette) + encoded)
    print(f"{OUTPUT.name}: {width}x{height}, {OUTPUT.stat().st_size} bytes, tip at {tip}")


if __name__ == "__main__":
    main()
