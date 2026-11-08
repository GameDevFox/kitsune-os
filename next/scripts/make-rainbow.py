#!/usr/bin/env python3
"""Writes `kernel/src/rainbow.rs`: the colors the orbiting square cycles through.

Usage: make-rainbow.py            Write the file
       make-rainbow.py --check    Exit 1 if the file isn't what this would write

The hues go round at an even perceived brightness, so none flashes brighter or darker
than the rest, which full-strength hues do: pure blue is about a tenth as bright as
pure yellow. They're chosen in OKLab, a color space where equal steps look equal: at a
fixed lightness, each hue is as saturated as the screen can show. That most saturated
color jumps around between hues, which showed as bands, so it's smoothed, only ever
downwards, so every color stays on the screen. The kernel can't use floating point,
so this works the table out once, and the file is checked in.
"""
import math
import sys
from pathlib import Path

NEXT = Path(__file__).resolve().parent.parent
OUTPUT = NEXT / "kernel/src/rainbow.rs"

# The square goes round these once a second, at 60 frames a second, so it shows about
# every fourth: more than that leaves room to slow it down without steps showing.
COUNT = 256
# OKLab lightness, from 0 for black to 1 for white. Lower makes yellow muddy, and
# higher washes out blue.
LIGHTNESS = 0.75
# How many hues either side the smoothing looks at: a running minimum keeps every color
# showable, then a running average of that softens the corners.
SMOOTHING = 24
# Bisection steps for the most saturated color, and an upper bound to start from.
SEARCH_STEPS = 40
MOST_CHROMA = 0.5
GAMUT_SLACK = 1e-9
FULL = 255
OPAQUE = 0xFF00_0000

# OKLab to linear sRGB, from Björn Ottosson's definition of OKLab.
TO_LMS = [(0.3963377774, 0.2158037573), (-0.1055613458, -0.0638541728), (-0.0894841775, -1.2914855480)]
TO_RGB = [
    (4.0767416621, -3.3077115913, 0.2309699292),
    (-1.2684380046, 2.6097574011, -0.3413193965),
    (-0.0041960863, -0.7034186147, 1.7076147010),
]
# The sRGB transfer function.
LINEAR_LIMIT, LINEAR_SCALE = 0.0031308, 12.92
GAMMA, GAMMA_SCALE, GAMMA_OFFSET = 2.4, 1.055, 0.055


def linear_rgb(chroma, hue):
    a, b = chroma * math.cos(hue), chroma * math.sin(hue)
    lms = [(LIGHTNESS + to_a * a + to_b * b) ** 3 for to_a, to_b in TO_LMS]
    return [sum(weight * value for weight, value in zip(row, lms)) for row in TO_RGB]


def showable(chroma, hue):
    return all(-GAMUT_SLACK <= value <= 1 + GAMUT_SLACK for value in linear_rgb(chroma, hue))


def most_chroma(hue):
    low, high = 0.0, MOST_CHROMA
    for _ in range(SEARCH_STEPS):
        middle = (low + high) / 2
        low, high = (middle, high) if showable(middle, hue) else (low, middle)
    return low


def encode(value):
    value = min(max(value, 0.0), 1.0)
    curved = value * LINEAR_SCALE if value <= LINEAR_LIMIT else GAMMA_SCALE * value ** (1 / GAMMA) - GAMMA_OFFSET
    return round(curved * FULL)


def around(values, index):
    return [values[(index + offset) % COUNT] for offset in range(-SMOOTHING, SMOOTHING + 1)]


def colors():
    """Each color as (red, green, blue), from 0 to 255."""
    hues = [2 * math.pi * index / COUNT for index in range(COUNT)]
    most = [most_chroma(hue) for hue in hues]
    floor = [min(around(most, index)) for index in range(COUNT)]
    smooth = [sum(around(floor, index)) / (2 * SMOOTHING + 1) for index in range(COUNT)]
    return [tuple(encode(value) for value in linear_rgb(chroma, hue)) for chroma, hue in zip(smooth, hues)]


def source():
    words = [OPAQUE | red << 16 | green << 8 | blue for red, green, blue in colors()]
    lines = ",\n".join("    " + ", ".join(f"0x{word:08X}" for word in words[at:at + 6]) for at in range(0, COUNT, 6))
    return f"""//! The colors the orbiting square cycles through, at an even brightness. Written by
//! `scripts/make-rainbow.py`, which says how they're chosen, so change that, not this.

use crate::video::Color;

pub const COUNT: usize = {COUNT};

pub const COLORS: [Color; COUNT] = [
{lines},
];
"""


def main():
    if sys.argv[1:] == ["--check"]:
        current = OUTPUT.exists() and OUTPUT.read_text() == source()
        print(f"{OUTPUT.name} is {'current' if current else 'out of date: run scripts/make-rainbow.py'}")
        sys.exit(0 if current else 1)
    OUTPUT.write_text(source())
    print(f"wrote {OUTPUT}")


if __name__ == "__main__":
    main()
