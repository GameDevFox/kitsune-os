#!/usr/bin/env python3
"""Tests for the config.txt editing in card.py. Run: python3 scripts/test_card.py"""

import unittest

from card import CardError, active_kernel, padded, switch_config

OLD = "enable_uart=1\n# kernel=comment.img\n\nkernel=kitsune.img\n"
NEW = "enable_uart=1\n# kernel=comment.img\n\nkernel=kitsune-next.img\narm_64bit=1\n"


class Padded(unittest.TestCase):
    def test_pads_with_zeros_to_exactly_the_size(self):
        self.assertEqual(padded(b"abc", 5), b"abc\0\0")

    def test_already_the_right_size(self):
        self.assertEqual(padded(b"abc", 3), b"abc")

    def test_too_big_is_refused(self):
        with self.assertRaises(CardError):
            padded(b"abcd", 3)


class SwitchConfig(unittest.TestCase):
    def test_switching_to_the_new_kernel(self):
        self.assertEqual(switch_config(OLD, "kitsune-next.img", True), NEW)

    def test_switching_back_restores_the_original_exactly(self):
        self.assertEqual(switch_config(NEW, "kitsune.img", False), OLD)

    def test_switching_twice_changes_nothing_more(self):
        once = switch_config(OLD, "kitsune-next.img", True)
        self.assertEqual(switch_config(once, "kitsune-next.img", True), once)

    def test_comments_are_left_alone(self):
        result = switch_config(OLD, "kitsune-next.img", True)
        self.assertIn("# kernel=comment.img", result)

    def test_no_kernel_line_is_refused(self):
        with self.assertRaises(CardError):
            switch_config("enable_uart=1\n", "x.img", False)

    def test_two_kernel_lines_are_refused(self):
        with self.assertRaises(CardError):
            switch_config("kernel=a.img\nkernel=b.img\n", "x.img", False)

    def test_crlf_is_refused(self):
        with self.assertRaises(CardError):
            switch_config("kernel=a.img\r\n", "x.img", False)


class ActiveKernel(unittest.TestCase):
    def test_reads_the_kernel_and_mode(self):
        self.assertEqual(active_kernel(OLD), (["kitsune.img"], False))
        self.assertEqual(active_kernel(NEW), (["kitsune-next.img"], True))


if __name__ == "__main__":
    unittest.main()
