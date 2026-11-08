#!/usr/bin/env python3
"""Checks Aki's PC-98 look. Run it directly: python3 scripts/test_pc98.py"""
import importlib.util
import re
import subprocess
import unittest
from pathlib import Path

import pc98

SCRIPTS = Path(__file__).resolve().parent
NEXT = SCRIPTS.parent
IMAGES = NEXT.parent / "kernel/image/aki"
MOUTH_LEFT, MOUTH_TOP = 368, 220
PINK = bytes((0xEE, 0xAA, 0xAA))
RIBBON_DARK = bytes((0x50, 0x5C, 0x5C))
BUTTON_BROWN = bytes((0x60, 0x20, 0x10))
CUFF_KHAKI = bytes((0x98, 0x88, 0x66))

# Positions in and out of each area, chosen by hand.
OUTSIDE_EVERY_AREA = (700, 100)
# The first is at the pattern's start when measured in 3-pixel blocks, the second in pixels.
BLOCK_X, BLOCK_Y = 696, 96
DOT_X, DOT_Y = 704, 104
IN_FACE = (340, 200)
IN_RIBBON = (380, 350)
RIBBON_TIP = (328, 388)
# The same point in the dither pattern as IN_FACE, so the two can be compared directly.
IN_WRIST = (180, 512)


def load(name):
    path = IMAGES / name
    size = subprocess.run(["magick", str(path), "-format", "%w %h", "info:"], capture_output=True, text=True, check=True)
    width, height = (int(value) for value in size.stdout.split())
    raw = subprocess.run(["magick", str(path), "-depth", "8", "rgba:-"], capture_output=True, check=True).stdout
    return width, height, raw


def all_phases(color, position):
    """The colors chosen across a whole dither pattern's worth of nearby positions."""
    left, top = position
    return {pc98.pick(*color, left + x, top + y) for y in range(pc98.BAYER_SIZE) for x in range(pc98.BAYER_SIZE)}


def pattern(color, position):
    """The colors chosen at each point of one dither pattern, in order."""
    left, top = position
    return [pc98.pick(*color, left + x, top + y) for y in range(pc98.BAYER_SIZE) for x in range(pc98.BAYER_SIZE)]


class Limits(unittest.TestCase):
    def test_the_palette_has_16_colors_of_4_bits_per_channel(self):
        codes = set(pc98.EVERYWHERE + pc98.PINKS)
        self.assertEqual(len(codes), 16)
        self.assertTrue(set(pc98.DARKS) <= codes)

    def test_a_picture_only_draws_palette_colors_and_skips_unsolid_pixels(self):
        solid = bytes((200, 120, 80, pc98.SOLID))
        clear = bytes((200, 120, 80, 0))
        colors = pc98.render(2, 1, solid + clear, 0, 0)
        self.assertIn(colors[0], {pc98.to_bytes(code) for code in pc98.EVERYWHERE + pc98.PINKS})
        self.assertIsNone(colors[1])


class Areas(unittest.TestCase):
    def test_pink_stays_out_of_ordinary_areas(self):
        pinks = {pc98.to_bytes(code) for code in pc98.PINKS}
        self.assertFalse(all_phases(PINK, OUTSIDE_EVERY_AREA) & pinks)

    def test_pink_appears_on_her_face(self):
        pinks = {pc98.to_bytes(code) for code in pc98.PINKS}
        self.assertTrue(all_phases(PINK, IN_FACE) & pinks)

    def test_the_ribbon_only_uses_dark_colors(self):
        darks = {pc98.to_bytes(code) for code in pc98.DARKS}
        self.assertTrue(all_phases(RIBBON_DARK, IN_RIBBON) <= darks)

    def test_the_tip_of_the_ribbon_only_uses_dark_colors(self):
        darks = {pc98.to_bytes(code) for code in pc98.DARKS}
        self.assertTrue(all_phases(RIBBON_DARK, RIBBON_TIP) <= darks)

    def test_the_same_dark_color_elsewhere_can_use_lighter_ones(self):
        darks = {pc98.to_bytes(code) for code in pc98.DARKS}
        self.assertFalse(all_phases(RIBBON_DARK, OUTSIDE_EVERY_AREA) <= darks)

    def test_a_warm_dark_color_on_the_ribbon_keeps_its_browns(self):
        darks = {pc98.to_bytes(code) for code in pc98.DARKS}
        self.assertFalse(all_phases(BUTTON_BROWN, IN_RIBBON) <= darks)

    def test_the_wrist_softens_pink_without_removing_it(self):
        pinks = {pc98.to_bytes(code) for code in pc98.PINKS}
        self.assertNotEqual(pattern(PINK, IN_WRIST), pattern(PINK, IN_FACE))
        self.assertTrue(all_phases(PINK, IN_WRIST) & pinks)

    def test_the_wrist_leaves_other_colors_alone(self):
        self.assertEqual(pattern(CUFF_KHAKI, IN_WRIST), pattern(CUFF_KHAKI, IN_FACE))


class Overlays(unittest.TestCase):
    def test_an_overlay_matches_a_true_composite_with_no_seam(self):
        base_width, base_height, base = load("base.png")
        width, height, mouth = load("mouth2.png")
        composite = bytearray(base)
        for y in range(height):
            start = ((MOUTH_TOP + y) * base_width + MOUTH_LEFT) * pc98.PIXEL_BYTES
            row = y * width * pc98.PIXEL_BYTES
            composite[start:start + width * pc98.PIXEL_BYTES] = mouth[row:row + width * pc98.PIXEL_BYTES]

        whole = pc98.render(base_width, base_height, composite, 0, 0)
        overlay = pc98.render(width, height, mouth, MOUTH_LEFT, MOUTH_TOP)
        for y in range(height):
            for x in range(width):
                self.assertEqual(overlay[y * width + x], whole[(MOUTH_TOP + y) * base_width + MOUTH_LEFT + x], (x, y))


class Blocks(unittest.TestCase):
    def test_a_box_rounds_out_to_whole_blocks(self):
        self.assertEqual(pc98.block_box(368, 220, 37, 27, 3), (366, 219, 39, 30))
        self.assertEqual(pc98.block_box(368, 220, 37, 27, 2), (368, 220, 38, 28))

    def test_every_block_is_one_color(self):
        base = load("base.png")
        mouth = load("mouth2.png")
        (_, _, width, height), colors = pc98.render_blocks(base, mouth, MOUTH_LEFT, MOUTH_TOP, 3)
        for y in range(0, height, 3):
            for x in range(0, width, 3):
                self.assertEqual({colors[(y + dy) * width + x + dx] for dy in range(3) for dx in range(3)}, {colors[y * width + x]})

    def test_a_blocky_overlay_matches_a_true_composite(self):
        base = load("base.png")
        mouth = load("mouth2.png")
        for scale in (2, 3, 4):
            whole = (base[0], base[1], bytes(pc98.composite(base, mouth, MOUTH_LEFT, MOUTH_TOP)))
            (_, _, whole_width, _), composite = pc98.render_blocks(base, whole, 0, 0, scale)
            (left, top, width, height), overlay = pc98.render_blocks(base, mouth, MOUTH_LEFT, MOUTH_TOP, scale)
            for y in range(height):
                for x in range(width):
                    self.assertEqual(overlay[y * width + x], composite[(top + y) * whole_width + left + x], (scale, x, y))

    def test_the_overlay_changes_what_the_base_shows(self):
        base = load("base.png")
        mouth = load("mouth2.png")
        (_, _, whole_width, _), plain = pc98.render_blocks(base, base, 0, 0, 3)
        (left, top, width, height), overlay = pc98.render_blocks(base, mouth, MOUTH_LEFT, MOUTH_TOP, 3)
        changed = sum(overlay[y * width + x] != plain[(top + y) * whole_width + left + x] for y in range(height) for x in range(width))
        self.assertGreater(changed, 0)

    def test_the_dither_pattern_is_measured_in_blocks(self):
        for i in range(pc98.BAYER_SIZE):
            for j in range(pc98.BAYER_SIZE):
                block = pc98.pick(*CUFF_KHAKI, BLOCK_X + 3 * i, BLOCK_Y + 3 * j, 3)
                dot = pc98.pick(*CUFF_KHAKI, DOT_X + i, DOT_Y + j)
                self.assertEqual(block, dot, (i, j))


class Slots(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location("make_assets", SCRIPTS / "make-assets.py")
        self.make_assets = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.make_assets)

    def test_every_picture_of_aki_uses_the_look(self):
        for name, source, _, reduction in self.make_assets.SLOTS:
            if source.startswith("aki/") and not name.startswith("aki-original"):
                self.assertIsInstance(reduction, self.make_assets.Pc98, name)

    def test_each_mouth_is_rendered_where_the_kernel_draws_it(self):
        pictures = (NEXT / "kernel/src/pictures.rs").read_text()
        found = re.findall(r"mouth: assets::(\w+), mouth_left: (\d+), mouth_top: (\d+)", pictures)
        self.assertEqual(len(found), 4)
        slots = {slot[0]: slot for slot in self.make_assets.SLOTS}
        original = next(item for item in found if item[0] == "AKI_ORIGINAL_MOUTH")
        self.assertEqual((int(original[1]), int(original[2])), (MOUTH_LEFT, MOUTH_TOP))
        found.remove(original)
        names = {"AKI_MOUTH": "aki-mouth2", "AKI_X2_MOUTH": "aki-x2-mouth", "AKI_X4_MOUTH": "aki-x4-mouth"}
        width, height, _ = load("mouth2.png")
        for constant, left, top in found:
            reduction = slots[names[constant]][3]
            box = pc98.block_box(reduction.left, reduction.top, width, height, reduction.scale)
            self.assertEqual(box[:2], (int(left), int(top)), constant)

    def test_slot_numbers_match_the_kernel(self):
        assets = (NEXT / "kernel/src/assets.rs").read_text()
        numbers = {name: int(value) for name, value in re.findall(r"pub const (\w+): usize = (\d+);", assets)}
        names = [slot[0] for slot in self.make_assets.SLOTS]
        for constant, name in [("AKI", "aki"), ("AKI_MOUTH", "aki-mouth2"), ("AKI_X2", "aki-x2"), ("AKI_X2_MOUTH", "aki-x2-mouth"), ("AKI_X4", "aki-x4"), ("AKI_X4_MOUTH", "aki-x4-mouth"), ("AKI_ORIGINAL", "aki-original"), ("AKI_ORIGINAL_MOUTH", "aki-original-mouth")]:
            self.assertEqual(numbers[constant], names.index(name), constant)

    def test_a_slot_position_reaches_the_dither(self):
        width, height, raw = load("mouth2.png")
        _, colors = self.make_assets.placed_colors(width, height, raw, self.make_assets.Pc98(MOUTH_LEFT, MOUTH_TOP))
        self.assertEqual(colors, pc98.render(width, height, raw, MOUTH_LEFT, MOUTH_TOP))
        self.assertNotEqual(colors, pc98.render(width, height, raw, 0, 0))


if __name__ == "__main__":
    unittest.main()
