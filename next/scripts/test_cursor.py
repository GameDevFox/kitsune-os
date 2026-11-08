#!/usr/bin/env python3
"""Checks that the kernel's cursor data comes from the image, and that the kernel agrees about it."""
import importlib.util
import re
import struct
import sys
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("make_cursor", HERE / "make-cursor.py")
make_cursor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(make_cursor)

WIDTH, HEIGHT, COLORS, TIP = make_cursor.render()
SIZE = make_cursor.SIZE_PIXELS
HEADER = "<HHBH"
RGB_BYTES = 3


def rgba(alpha):
    return bytes((1, 2, 3, alpha)) * (SIZE * SIZE)


class Cursor(unittest.TestCase):
    def test_the_tip_is_on_the_cursor(self):
        self.assertIsNotNone(COLORS[TIP[1] * WIDTH + TIP[0]])

    def test_the_kernels_data_is_what_the_image_holds(self):
        output = make_cursor.OUTPUT.read_bytes()
        width, height, index_size, count = struct.unpack_from(HEADER, output)
        self.assertEqual((width, height), (WIDTH, HEIGHT))
        start = struct.calcsize(HEADER)
        palette = [output[start + index * RGB_BYTES:start + (index + 1) * RGB_BYTES] for index in range(count)]
        encoded = output[start + count * RGB_BYTES:]
        decoded = make_cursor.make_assets.decode_symbols(encoded, index_size)
        shown = [palette[symbol - 1] if symbol else None for symbol in decoded]
        self.assertEqual(shown, COLORS, "cursor.bin is out of date: run scripts/make-cursor.py")

    def test_the_kernel_agrees_about_the_size_and_tip(self):
        source = (HERE.parent / "kernel/src/cursor.rs").read_text()
        constant = lambda name: int(re.search(rf"const {name}: usize = (\d+);", source).group(1))
        self.assertEqual((constant("WIDTH"), constant("HEIGHT")), (WIDTH, HEIGHT))
        self.assertEqual((constant("TIP_X"), constant("TIP_Y")), TIP)

    def test_opaque_and_transparent_pixels_are_read_correctly(self):
        self.assertEqual(make_cursor.colors_from(SIZE, SIZE, rgba(make_cursor.OPAQUE)), [bytes((1, 2, 3))] * (SIZE * SIZE))
        self.assertEqual(make_cursor.colors_from(SIZE, SIZE, rgba(make_cursor.TRANSPARENT)), [None] * (SIZE * SIZE))

    def test_a_partly_transparent_pixel_is_refused(self):
        with self.assertRaises(SystemExit):
            make_cursor.colors_from(SIZE, SIZE, rgba(1))

    def test_the_wrong_size_is_refused(self):
        with self.assertRaises(SystemExit):
            make_cursor.colors_from(SIZE - 1, SIZE, rgba(make_cursor.OPAQUE))


if __name__ == "__main__":
    unittest.main()
