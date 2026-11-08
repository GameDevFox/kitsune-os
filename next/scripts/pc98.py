"""Aki's PC-98 look: 16 hand-picked colors at 4 bits per channel, and ordered dithering.

The dither only depends on a pixel's color and its position in the base image, so
an overlay rendered at its own position matches the base around it exactly, and
can't leave a seam. `test_pc98.py` checks that against a true composite.
"""
import subprocess
from functools import lru_cache

# Positions below are in the pixels of Aki's base image.
BASE_WIDTH, BASE_HEIGHT = 775, 1521

# Each color is 3 hex digits, as on the PC-98.
PINKS = ("c67", "eaa")
EVERYWHERE = ("000", "233", "311", "532", "753", "986", "ccb", "9aa", "b64", "e96", "f50", "db9", "fed", "fff")
DARKS = ("000", "233", "311")

# The only places pink may appear, as it otherwise bleeds into her hair and clothes.
PINK_AREAS = (
    # Her lower face: cheeks and mouth, kept clear of the hair strand and her neck.
    ((326, 186), (398, 196), (456, 186), (450, 205), (430, 218), (406, 230), (402, 240), (394, 254), (370, 252), (346, 238), (330, 216)),
    # The hand held out, and its wrist.
    ((0, 455), (40, 420), (110, 420), (165, 450), (215, 490), (205, 535), (160, 530), (120, 500), (40, 510), (0, 495)),
    # The lower hand, and its wrist.
    ((570, 715), (610, 715), (660, 770), (712, 790), (712, 818), (660, 815), (610, 790), (575, 750)),
)

# Her ribbon, with a notch for the tip at its lower left. Dark, cool pixels in here only use dark colors, which keeps light dots
# out of it, while the lace between its loops and her warm-colored button are unaffected.
DARK_AREAS = (((335, 295), (440, 295), (440, 412), (335, 412), (335, 398), (322, 398), (322, 370), (335, 370)),)
# Perceptual lightness out of 100, below which a pixel counts as dark.
DARK_MAX_LIGHTNESS = 40
# Lab's green-red axis, above which a pixel counts as warm.
DARK_MAX_RED = 8

# Her held-out wrist. Pink pixels are mixed toward skin here, which turns a solid
# pink stripe into a dithered blush.
SOFT_AREAS = (((135, 470), (215, 490), (205, 535), (160, 530), (135, 505)),)
SOFT_TOWARD = (0xFF, 0xEE, 0xDD)
SOFT_PERCENT = 30
# Lab's green-red axis, above which a pixel counts as pink, so her cuff is left alone.
SOFT_MIN_RED = 18

BAYER = (
    (0, 32, 8, 40, 2, 34, 10, 42), (48, 16, 56, 24, 50, 18, 58, 26),
    (12, 44, 4, 36, 14, 46, 6, 38), (60, 28, 52, 20, 62, 30, 54, 22),
    (3, 35, 11, 43, 1, 33, 9, 41), (51, 19, 59, 27, 49, 17, 57, 25),
    (15, 47, 7, 39, 13, 45, 5, 37), (63, 31, 55, 23, 61, 29, 53, 21),
)
BAYER_SIZE = 8
BAYER_LEVELS = 64
# How far the pattern nudges a color, in 8-bit channel values.
SPREAD = 64

PIXEL_BYTES = 4
SOLID = 255
# Multiplying a 4-bit channel by this spreads it across 8 bits: 0xF becomes 0xFF.
CHANNEL_STEP = 0x11


def to_bytes(code):
    return bytes(int(digit, 16) * CHANNEL_STEP for digit in code)


def to_lab(color):
    """Converts sRGB to Lab, where distances are close to how different colors look."""

    def linear(value):
        value /= 255
        return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4

    def curve(value):
        return value ** (1 / 3) if value > 0.008856 else 7.787 * value + 16 / 116

    r, g, b = (linear(value) for value in color)
    x = curve((0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047)
    y = curve(0.2126 * r + 0.7152 * g + 0.0722 * b)
    z = curve((0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883)
    return 116 * y - 16, 500 * (x - y), 200 * (y - z)


@lru_cache(maxsize=None)
def area_mask(areas):
    """Returns one byte per base image pixel, which is nonzero inside the areas."""
    command = ["magick", "-size", f"{BASE_WIDTH}x{BASE_HEIGHT}", "xc:black", "-fill", "white"]
    for area in areas:
        command += ["-draw", "polygon " + " ".join(f"{x},{y}" for x, y in area)]
    return subprocess.run(command + ["-depth", "8", "gray:-"], capture_output=True, check=True).stdout


def lab_table(codes):
    return [(to_bytes(code), to_lab(to_bytes(code))) for code in codes]


CHOICES = {
    "normal": lab_table(EVERYWHERE),
    "pink": lab_table(EVERYWHERE + PINKS),
    "dark": lab_table(DARKS),
}


@lru_cache(maxsize=None)
def nearest(red, green, blue, choice):
    target = to_lab((max(0, min(255, red)), max(0, min(255, green)), max(0, min(255, blue))))
    return min(CHOICES[choice], key=lambda entry: sum((a - b) ** 2 for a, b in zip(entry[1], target)))[0]


def pick(red, green, blue, x, y, scale=1):
    """Returns the color to draw for a source color at a position in the base image.
    With a scale, the dither pattern is measured in blocks of that many pixels."""
    at = y * BASE_WIDTH + x
    if area_mask(SOFT_AREAS)[at] and to_lab((red, green, blue))[1] > SOFT_MIN_RED:
        red, green, blue = ((value * (100 - SOFT_PERCENT) + toward * SOFT_PERCENT) // 100 for value, toward in zip((red, green, blue), SOFT_TOWARD))

    choice = "pink" if area_mask(PINK_AREAS)[at] else "normal"
    if area_mask(DARK_AREAS)[at]:
        lightness, redness, _ = to_lab((red, green, blue))
        if lightness < DARK_MAX_LIGHTNESS and redness < DARK_MAX_RED:
            choice = "dark"

    nudge = (BAYER[y // scale % BAYER_SIZE][x // scale % BAYER_SIZE] * 2 + 1 - BAYER_LEVELS) * SPREAD // (2 * BAYER_LEVELS)
    return nearest(red + nudge, green + nudge, blue + nudge, choice)


def render(width, height, raw, left, top):
    """Returns the color to draw for each pixel, or None to skip it, for a picture at
    a position in the base image. The base image itself is at 0, 0."""
    colors = []
    for y in range(height):
        for x in range(width):
            at = (y * width + x) * PIXEL_BYTES
            solid = raw[at + 3] == SOLID
            colors.append(pick(raw[at], raw[at + 1], raw[at + 2], left + x, top + y) if solid else None)
    return colors


def block_box(left, top, width, height, scale):
    """Returns a picture's position and size, rounded out to whole blocks."""
    new_left, new_top = left // scale * scale, top // scale * scale
    right, bottom = (left + width + scale - 1) // scale * scale, (top + height + scale - 1) // scale * scale
    return new_left, new_top, right - new_left, bottom - new_top


def composite(base, overlay, left, top):
    """Returns a copy of the base's pixels, with the overlay's solid pixels on top."""
    base_width, _, base_raw = base
    width, height, raw = overlay
    pixels = bytearray(base_raw)
    for y in range(height):
        for x in range(width):
            at = (y * width + x) * PIXEL_BYTES
            if raw[at + 3] == SOLID:
                target = ((top + y) * base_width + left + x) * PIXEL_BYTES
                pixels[target:target + PIXEL_BYTES] = raw[at:at + PIXEL_BYTES]
    return pixels


def render_blocks(base, overlay, left, top, scale):
    """Renders a picture at a position in the base image, in blocks of `scale` pixels.

    Returns its rounded-out position and size, and a color per full-size pixel, or
    None to skip it. Blocks are averaged from the picture on top of the base, so ones
    the picture only partly covers match what a true composite would show. A block is
    solid if at least half its pixels are, and its color is the average of those.
    """
    base_width, base_height, _ = base
    pixels = composite(base, overlay, left, top)
    box_left, box_top, box_width, box_height = block_box(left, top, overlay[0], overlay[1], scale)
    colors = [None] * (box_width * box_height)
    for block_y in range(box_top, box_top + box_height, scale):
        for block_x in range(box_left, box_left + box_width, scale):
            total = [0, 0, 0]
            solid = 0
            for y in range(block_y, min(block_y + scale, base_height)):
                for x in range(block_x, min(block_x + scale, base_width)):
                    at = (y * base_width + x) * PIXEL_BYTES
                    if pixels[at + 3] == SOLID:
                        solid += 1
                        for channel in range(3):
                            total[channel] += pixels[at + channel]
            if solid * 2 < scale * scale:
                continue
            color = pick(*(value // solid for value in total), block_x, block_y, scale)
            for y in range(block_y - box_top, block_y - box_top + scale):
                start = y * box_width + block_x - box_left
                colors[start:start + scale] = [color] * scale
    return (box_left, box_top, box_width, box_height), colors
