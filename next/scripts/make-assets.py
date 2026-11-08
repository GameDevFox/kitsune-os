#!/usr/bin/env python3
"""Builds the image region, and the full image for the SD card.

Usage: make-assets.py KERNEL_IMAGE OUTPUT_IMAGE

Images live at a fixed place in RAM, after the kernel, so `cargo pi` can replace
the kernel without touching them. Each image has a fixed slot with spare room, so
a slightly bigger version doesn't move the others. Slot order must match the
indexes in `kernel/src/assets.rs`.

Region layout, little-endian:
  8 bytes   the magic bytes below
  4 bytes   number of slots
  per slot  4 bytes offset from the region's start, 4 bytes length

The last slot is a sound clip rather than an image, in `tools/voice.py`'s form, and
empty on a machine with no `voice.local.json`.

Each image:
  2 bytes width, 2 bytes height
  1 byte index size: 0 for full-color pixels, or 1 or 2 for palette indexes
  2 bytes palette count, then count x 3 bytes of red, green, blue
  run-length packets of "symbols", a symbol being one pixel or one index:
    1-254, symbol                    A run of identical symbols
    0xFF, length (0 means 256), ...  That many raw symbols
A full-color pixel is 4 bytes: red, green, blue, alpha, and is skipped unless fully
solid. Index 0 means skip this pixel, and index N means palette entry N - 1.
"""
import importlib.util
import struct
import subprocess
import sys
from collections import Counter, namedtuple
from pathlib import Path

# `check_images.py` loads this file by its path, which doesn't make its neighbors importable.
SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))
import pc98

NEXT = SCRIPTS.parent
IMAGES = NEXT.parent / "kernel/image"
CACHE = NEXT / "target/assets.bin"


def load_voice():
    spec = importlib.util.spec_from_file_location("voice", NEXT / "tools/voice.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


voice = load_voice()

# Change this when the format changes, so a kernel can't misread older images.
MAGIC = b"KITSASS2"
KILOBYTE = 1024
MEGABYTE = 1024 * KILOBYTE

# Must match the kernel's link address, and ASSETS_BASE and ASSETS_SIZE in `board/raspi4/mod.rs`.
KERNEL_ADDRESS = 0x80000
ASSETS_ADDRESS = 0x200000
# The region must stay well below 0x1000000, which something overwrites during boot.
ASSETS_LIMIT = 6 * MEGABYTE

SLOT_ALIGNMENT = 64 * KILOBYTE
TABLE_SPACE = 64 * KILOBYTE

# Aki's look, given the picture's position in her base image, and how many pixels
# wide each dot is. A dot wider than 1 is stored as that many ordinary pixels, which
# costs storage but needs nothing from the decoder.
Pc98 = namedtuple("Pc98", "left top scale", defaults=(1,))
AKI_BASE = "aki/base.png"
BIG_DOT_ROOM = 512 * KILOBYTE

# Name, source, room to reserve, and how to reduce its colors: palette sizes to go
# through in order, Pc98, or None for full color. The room leaves space for later versions.
SLOTS = [
    ("logo", "logo.png", 128 * KILOBYTE, (16, 4)),
    ("aki", AKI_BASE, 2 * MEGABYTE, Pc98(0, 0)),
    ("aki-glasses", "aki/glasses.png", 64 * KILOBYTE, Pc98(308, 172)),
    ("aki-no-glasses", "aki/no-glasses.png", 64 * KILOBYTE, Pc98(308, 172)),
    ("aki-mouth0", "aki/mouth0.png", 16 * KILOBYTE, Pc98(368, 220)),
    ("aki-mouth1", "aki/mouth1.png", 16 * KILOBYTE, Pc98(368, 220)),
    ("aki-mouth2", "aki/mouth2.png", 16 * KILOBYTE, Pc98(368, 220)),
    ("aki-x2", AKI_BASE, BIG_DOT_ROOM, Pc98(0, 0, 2)),
    ("aki-x2-mouth", "aki/mouth2.png", 16 * KILOBYTE, Pc98(368, 220, 2)),
    ("aki-x4", AKI_BASE, BIG_DOT_ROOM, Pc98(0, 0, 4)),
    ("aki-x4-mouth", "aki/mouth2.png", 16 * KILOBYTE, Pc98(368, 220, 4)),
    ("aki-original", AKI_BASE, 1600 * KILOBYTE, None),
    ("aki-original-mouth", "aki/mouth2.png", 16 * KILOBYTE, None),
]

# After the images, a sound clip in `tools/voice.py`'s form: the line `voice.local.json`
# names, so it's on the card, or nothing if this machine has no such file. Its slot is
# `VOICE` in `kernel/src/assets.rs`. This much is over 5 s.
VOICE_SLOT = len(SLOTS)
VOICE_ROOM = 512 * KILOBYTE
# The table's words, and where a slot's length sits in its entry.
WORD_BYTES = 4
ENTRY_BYTES = 2 * WORD_BYTES
LENGTH_AT = WORD_BYTES

ESCAPE = 0xFF
MAX_RUN = 254
MAX_RAW = 256
PIXEL_BYTES = 4
FULL_COLOR = 0
SOLID = 255
# The bytes a run costs when it interrupts raw data: its count, plus a new raw header.
RUN_OVERHEAD = 3


def worth_a_run(length, symbol_size):
    """Whether a repeat is smaller as a run than left inside raw data."""
    return length * symbol_size > symbol_size + RUN_OVERHEAD


def encode_symbols(pixels):
    """Run-length encodes a list of equally sized symbols."""
    symbol_size = len(pixels[0])
    output = bytearray()
    pending = []

    def flush():
        for start in range(0, len(pending), MAX_RAW):
            chunk = pending[start:start + MAX_RAW]
            output.append(ESCAPE)
            output.append(len(chunk) % MAX_RAW)
            output.extend(b"".join(chunk))
        pending.clear()

    index = 0
    while index < len(pixels):
        pixel = pixels[index]
        end = index + 1
        while end < len(pixels) and end - index < MAX_RUN and pixels[end] == pixel:
            end += 1
        length = end - index
        if worth_a_run(length, symbol_size):
            flush()
            output.append(length)
            output.extend(pixel)
        else:
            pending.extend([pixel] * length)
        index = end
    flush()
    return bytes(output)


def decode_symbols(data, symbol_size):
    """Reverses `encode_symbols`, so that the encoder can be checked."""
    output = bytearray()
    index = 0
    while index < len(data):
        first = data[index]
        if first == ESCAPE:
            length = data[index + 1] or MAX_RAW
            end = index + 2 + length * symbol_size
            output += data[index + 2:end]
            index = end
        else:
            output += data[index + 1:index + 1 + symbol_size] * first
            index += 1 + symbol_size
    return bytes(output)


def load_rgba(path):
    size = subprocess.run(["magick", str(path), "-format", "%w %h", "info:"], capture_output=True, text=True, check=True)
    width, height = (int(value) for value in size.stdout.split())
    raw = subprocess.run(["magick", str(path), "-depth", "8", "rgba:-"], capture_output=True, check=True).stdout
    assert len(raw) == width * height * PIXEL_BYTES, path
    return width, height, raw


def nearest(color, choices):
    return min(choices, key=lambda choice: sum((a - b) ** 2 for a, b in zip(choice, color)))


def reduce_colors(raw, steps):
    """Returns the RGB color to draw for each pixel, or None to skip it.

    Reduces through each palette size in turn, keeping the image's most common
    colors. Going straight to 4 colors turned mid-gray edge pixels into orange,
    because in raw RGB math a mid-gray can be closer to a bright color than to
    black or white. Stepping through 16 colors first avoids that on the logo. It
    relies on what's in the top 16, so another image may need something better.
    """
    pixels = [raw[i:i + PIXEL_BYTES] for i in range(0, len(raw), PIXEL_BYTES)]
    solid = Counter(pixel[:3] for pixel in pixels if pixel[3] == SOLID)
    table = {color: color for color in solid}
    for size in steps:
        palette = [color for color, _ in solid.most_common(size)]
        table = {color: nearest(reduced, palette) for color, reduced in table.items()}
    return [table[pixel[:3]] if pixel[3] == SOLID else None for pixel in pixels]


def placed_colors(width, height, raw, reduction):
    """Returns a slot's left and top relative to its base, its width and height, and
    the RGB color to draw for each pixel, or None to skip it."""
    if reduction is None:
        pixels = (raw[i:i + PIXEL_BYTES] for i in range(0, len(raw), PIXEL_BYTES))
        return (0, 0, width, height), [pixel[:3] if pixel[3] == SOLID else None for pixel in pixels]
    if not isinstance(reduction, Pc98):
        return (0, 0, width, height), reduce_colors(raw, reduction)
    left, top, scale = reduction
    if scale == 1:
        return (left, top, width, height), pc98.render(width, height, raw, left, top)
    return pc98.render_blocks(load_rgba(IMAGES / AKI_BASE), (width, height, raw), left, top, scale)


def encode_image(width, height, raw, reduction):
    """Returns the image's bytes and size, checking that they decode back correctly."""
    if reduction is None:
        pixels = [raw[i:i + PIXEL_BYTES] for i in range(0, len(raw), PIXEL_BYTES)]
        encoded = encode_symbols(pixels)
        if decode_symbols(encoded, PIXEL_BYTES) != raw:
            raise SystemExit("encoding doesn't decode back to the original")
        return struct.pack("<HHBH", width, height, FULL_COLOR, 0) + encoded, width, height

    (_, _, width, height), colors = placed_colors(width, height, raw, reduction)
    palette = [color for color, _ in Counter(c for c in colors if c is not None).most_common()]
    index_of = {color: number + 1 for number, color in enumerate(palette)}
    index_of[None] = 0
    index_size = 1 if len(palette) < 256 else 2
    symbols = [index_of[color].to_bytes(index_size, "little") for color in colors]
    encoded = encode_symbols(symbols)
    if decode_symbols(encoded, index_size) != b"".join(symbols):
        raise SystemExit("encoding doesn't decode back to the original")
    header = struct.pack("<HHBH", width, height, index_size, len(palette))
    return header + b"".join(palette) + encoded, width, height


def align(value):
    return (value + SLOT_ALIGNMENT - 1) // SLOT_ALIGNMENT * SLOT_ALIGNMENT


def build_region():
    region = bytearray(TABLE_SPACE)
    entries = []
    offset = TABLE_SPACE

    def place(name, blob, room, size):
        nonlocal offset
        if len(blob) > room:
            raise SystemExit(f"{name}: {len(blob)} bytes doesn't fit its {room}-byte slot")
        print(f"  {name:16} {size:9} {len(blob) // KILOBYTE:>5} KB of {room // KILOBYTE} KB, at {ASSETS_ADDRESS + offset:#x}")
        entries.append((offset, len(blob)))
        region.extend(bytes(offset - len(region)))
        region.extend(blob)
        offset = align(offset + room)

    for name, source, room, reduction in SLOTS:
        width, height, raw = load_rgba(IMAGES / source)
        blob, width, height = encode_image(width, height, raw, reduction)
        place(name, blob, room, f"{width:>4}x{height:<4}")
    line, clip, _ = voice.card_clip()
    seconds = (len(clip) - voice.HEADER_BYTES) / voice.SAMPLE_BYTES / voice.SAMPLE_RATE if clip else 0
    place("voice", clip, VOICE_ROOM, f"{seconds:4.2f} s" if clip else "none")
    if line:
        print(f"  (the voice is {line}, from {voice.SETTINGS.name})")

    table = MAGIC + struct.pack("<I", len(entries))
    for entry in entries:
        table += struct.pack("<II", *entry)
    assert len(table) <= TABLE_SPACE
    region[:len(table)] = table

    if ASSETS_ADDRESS + offset > ASSETS_ADDRESS + ASSETS_LIMIT:
        raise SystemExit(f"slots need {offset} bytes, past the {ASSETS_LIMIT}-byte limit")
    return bytes(region)


def cached_voice_bytes():
    cache = CACHE.read_bytes()
    entry = len(MAGIC) + WORD_BYTES + VOICE_SLOT * ENTRY_BYTES + LENGTH_AT
    if cache[:len(MAGIC)] != MAGIC or len(cache) < entry + WORD_BYTES:
        return None
    return struct.unpack_from("<I", cache, entry)[0]


def sources_changed():
    if not CACHE.exists():
        return True
    built = CACHE.stat().st_mtime
    sources = [Path(__file__), SCRIPTS / "pc98.py", NEXT / "tools/voice.py"]
    sources += [IMAGES / slot[1] for slot in SLOTS]
    if voice.SETTINGS.exists():
        sources += voice.sources(voice.settings())
    # Making or removing the settings changes nothing's time, so compare what was built.
    cached = cached_voice_bytes()
    if cached is None or (cached > 0) != voice.SETTINGS.exists():
        return True
    return any(path.stat().st_mtime > built for path in sources)


def main():
    kernel_path, output_path = Path(sys.argv[1]), Path(sys.argv[2])
    if sources_changed():
        print("encoding images:")
        CACHE.write_bytes(build_region())

    kernel = kernel_path.read_bytes()
    room = ASSETS_ADDRESS - KERNEL_ADDRESS
    if len(kernel) > room:
        raise SystemExit(f"the kernel is {len(kernel)} bytes, but only {room} fit before the images")
    output_path.write_bytes(kernel + bytes(room - len(kernel)) + CACHE.read_bytes())


if __name__ == "__main__":
    main()
