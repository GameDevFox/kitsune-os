#!/usr/bin/env python3
"""Reads the text on the real Pi's screen, from the HDMI capture.

Each character cell in a captured frame is matched against the kernel's font, so
kernel output can be checked exactly, and not just looked at.

Usage: screen_text.py [--server HOST:PORT]
"""
import subprocess
import sys
import urllib.request
from pathlib import Path

NEXT = Path(__file__).resolve().parent.parent
FONT = NEXT / "kernel/src/font-16x30.bin"
WIDTH, HEIGHT = 1920, 1080
GLYPH_WIDTH, GLYPH_HEIGHT = 16, 30
# The part of the screen the kernel uses for text.
COLUMNS, ROWS = 72, 17
BYTES_PER_GLYPH = GLYPH_HEIGHT * 2
FIRST_PRINTABLE, LAST_PRINTABLE = 0x20, 0x7E
# Halfway between the capture's black and the kernel's text color.
LIT_LEVEL = 110
# The capture card shows everything this many pixels right of where it was drawn,
# leaving a dark line down the left edge.
CAPTURE_X_SHIFT = 1
DEFAULT_SERVER = "localhost:8766"


def load_glyphs():
    """Returns each printable character's pixels, as one big integer of bits."""
    font = FONT.read_bytes()
    glyphs = {}
    for code in range(FIRST_PRINTABLE, LAST_PRINTABLE + 1):
        data = font[code * BYTES_PER_GLYPH:(code + 1) * BYTES_PER_GLYPH]
        glyphs[chr(code)] = int.from_bytes(data, "big")
    return glyphs


GLYPHS = load_glyphs()


def capture(server=DEFAULT_SERVER):
    with urllib.request.urlopen(f"http://{server}/frame.jpg", timeout=5) as response:
        jpeg = response.read()
    return subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "mjpeg", "-i", "-",
         "-f", "rawvideo", "-pix_fmt", "gray", "-"],
        input=jpeg, capture_output=True, check=True,
    ).stdout


def read_cell(gray, column, row):
    left, top = column * GLYPH_WIDTH, row * GLYPH_HEIGHT
    bits = 0
    for y in range(top, top + GLYPH_HEIGHT):
        for x in range(left, left + GLYPH_WIDTH):
            bits = bits << 1 | (gray[y * WIDTH + min(x + CAPTURE_X_SHIFT, WIDTH - 1)] > LIT_LEVEL)
    return min(GLYPHS, key=lambda char: (GLYPHS[char] ^ bits).bit_count())


def read_screen(server=DEFAULT_SERVER):
    """Returns the text rows, with trailing spaces removed."""
    gray = capture(server)
    return ["".join(read_cell(gray, column, row) for column in range(COLUMNS)).rstrip() for row in range(ROWS)]


if __name__ == "__main__":
    address = sys.argv[2] if len(sys.argv) == 3 and sys.argv[1] == "--server" else DEFAULT_SERVER
    for line in read_screen(address):
        print(line)
