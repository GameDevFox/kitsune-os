#!/usr/bin/env python3
"""Checks that the kernel draws its images correctly.

  check_images.py qemu    Boots the full image in QEMU, presses each key that changes
                          the pictures, and compares pixels exactly each time
  check_images.py pi      Compares shapes in the real Pi's HDMI capture

Expected pixels come from the source PNGs, reduced the way the image slots say,
under the kernel's rule of skipping anything that isn't fully solid. So this checks
drawing rather than the look, which `scripts/test_pc98.py` covers. Positions must
match `kernel/src/main.rs`.
"""
import importlib.util
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

import numpy as np

import spin_model

NEXT = Path(__file__).resolve().parent.parent
IMAGES = NEXT.parent / "kernel/image"
WIDTH, HEIGHT = 1920, 1080
LOGO = ("logo", 1166, 144)
AKI_LEFT, AKI_TOP = 1152, 0
MOUTH_LEFT, MOUTH_TOP = 368, 220
AKI_VERSIONS = {
    b"1": ("aki-original", "aki-original-mouth"),
    b"2": ("aki", "aki-mouth2"),
    b"3": ("aki-x2", "aki-x2-mouth"),
    b"4": ("aki-x4", "aki-x4-mouth"),
}
# Aki starts as the original. Its mouth is full size, so it keeps no position of its own.
STARTUP = [LOGO, ("aki-original", AKI_LEFT, AKI_TOP), ("aki-original-mouth", AKI_LEFT + MOUTH_LEFT, AKI_TOP + MOUTH_TOP)]
# Regions to compare, which avoid the text. The square circles the logo, through Aki,
# in a ring: `square.rs`'s radius around the logo's center, give or take the square's
# half diagonal and a little more. The logo's corners are well inside it. On the Pi,
# where colors aren't exact, the ring is skipped.
REGIONS = [("logo", 1166, 144, 168, 256), ("Aki", 1152, 0, 768, 1080)]
ORBIT_CENTER = (1166 + 168 // 2, 144 + 256 // 2)
ORBIT_RADIUS = 190
ORBIT_MARGIN = 32
# The square cycles through `scripts/make-rainbow.py`'s colors. In QEMU, a pixel in the
# ring may be exactly one of them, but otherwise must be right.
BOOT_SECONDS = 5
# Where tests move the cursor: over Aki, then well away from everything compared.
CURSOR_START = (960, 540)
CURSOR_OVER_AKI = (1500, 500)
CURSOR_AWAY = (700, 300)
CUBE_KEY = b"5"
# QEMU has no PWM, so this checks that sound there can't hang the kernel: a tone, and
# a clip, which QEMU never has loaded.
TONE_KEY = b"7"
CLIP_KEY = b"8"
# The cube's square of the screen, and how much of it a cube covers at the very least.
# Where `cube.rs` puts it: in the logo's place, as wide as the logo.
CUBE_REGION = (1166, 188, 168, 168)
CUBE_MIN_LIT = 0.1
KEY_SECONDS = 2
CLEAR_KEY = b"0"
# The capture card shifts everything one pixel right. Its colors aren't exact either:
# pure white reads about 210, and bright red about 219. So on the Pi, only whether
# each pixel is lit or dark gets compared, and QEMU checks the exact colors.
CAPTURE_X_SHIFT = 1
LIT_LEVEL = 70
CAPTURE_MAX_WRONG = 0.03


def load_script(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), NEXT / f"scripts/{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


MAKE_ASSETS = load_script("make-assets")
SQUARE_COLORS = load_script("make-rainbow").colors()
SQUARE_RGB = {bytes(color) for color in SQUARE_COLORS}
# The same, packed as numbers, to look pixels up in all at once.
SQUARE_PACKED = np.array([red << 16 | green << 8 | blue for red, green, blue in SQUARE_COLORS])
# Screens are seconds apart, and the colors go round in about 4 s, so a square that
# changes color shows several, and one stuck on a color shows one.
MIN_SQUARE_COLORS = 3
# A color counts as the square's if it covers at least this much of the square's 40 by
# 40, not if a few of Aki's pixels happen to match one.
SQUARE_MIN_PIXELS = 40 * 40 // 2
# Each slot's source and how it's reduced, from the table the images are built with.
SLOTS = {slot[0]: slot for slot in MAKE_ASSETS.SLOTS}


def load(path):
    size = subprocess.run(["magick", str(path), "-format", "%w %h", "info:"], capture_output=True, text=True, check=True)
    width, height = (int(value) for value in size.stdout.split())
    raw = subprocess.run(["magick", str(path), "-depth", "8", "rgba:-"], capture_output=True, check=True).stdout
    return width, height, raw


def crc16(data):
    crc = 0xFFFF
    for byte in data:
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc


def packet(message):
    """A separate encoder from the Rust one, for messages short enough for one COBS run."""
    plain = message + crc16(message).to_bytes(2, "little")
    encoded = bytearray([0])
    code_at = 0
    for byte in plain:
        if byte == 0:
            encoded[code_at] = len(encoded) - code_at
            code_at = len(encoded)
            encoded.append(0)
        else:
            encoded.append(byte)
    encoded[code_at] = len(encoded) - code_at
    return b"\0" + bytes(encoded) + b"\0"


def mouse_packet(dx, dy):
    return packet(bytes([1]) + dx.to_bytes(2, "little", signed=True) + dy.to_bytes(2, "little", signed=True))


def key_packet(key):
    return packet(bytes([2]) + key)


def corrupted(whole):
    return whole[:2] + bytes([whole[2] ^ 0x40]) + whole[3:]


def with_cursor(canvas, tip):
    """Draws the cursor on a screen the way the kernel does."""
    spec = importlib.util.spec_from_file_location("make_cursor", NEXT / "scripts/make-cursor.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    width, height, colors, sprite_tip = module.render()
    left, top = tip[0] - sprite_tip[0], tip[1] - sprite_tip[1]
    for y in range(height):
        for x in range(width):
            color = colors[y * width + x]
            if color is not None and 0 <= left + x < WIDTH and 0 <= top + y < HEIGHT:
                at = ((top + y) * WIDTH + left + x) * 3
                canvas[at:at + 3] = color
    return canvas


def expected(pictures):
    """The screen for pictures drawn in order, each at a position plus its slot's own."""
    canvas = bytearray(WIDTH * HEIGHT * 3)
    for name, base_left, base_top in pictures:
        _, source, _, reduction = SLOTS[name]
        width, height, raw = load(IMAGES / source)
        (left, top, width, height), colors = MAKE_ASSETS.placed_colors(width, height, raw, reduction)
        left += base_left
        top += base_top
        for y in range(min(height, HEIGHT - top)):
            at = ((top + y) * WIDTH + left) * 3
            for x in range(min(width, WIDTH - left)):
                color = colors[y * width + x]
                if color is not None:
                    canvas[at + x * 3:at + x * 3 + 3] = color
    return canvas


def in_orbit(x, y):
    distance_squared = (x - ORBIT_CENTER[0]) ** 2 + (y - ORBIT_CENTER[1]) ** 2
    return (ORBIT_RADIUS - ORBIT_MARGIN) ** 2 <= distance_squared <= (ORBIT_RADIUS + ORBIT_MARGIN) ** 2


def in_logo(x, y):
    """Whether a pixel is in the logo's box, with the rows above and below that its near
    edge reaches as it turns, which Aki's region leaves to the logo's own check."""
    _, left, top, width, height = REGIONS[0]
    extra = spin_model.EXTRA_ROWS
    return left <= x < left + width and top - extra <= y < top + height + extra


def compare(want, got):
    """Returns each region's name and fraction of pixels that aren't exactly right."""
    results = []
    for name, left, top, width, height in REGIONS:
        wrong = total = 0
        for y in range(top, top + height):
            for x in range(left, min(left + width, WIDTH)):
                at = (y * WIDTH + x) * 3
                if name != "logo" and in_logo(x, y):
                    continue
                if in_orbit(x, y) and got[at:at + 3] in SQUARE_RGB:
                    continue
                total += 1
                wrong += want[at:at + 3] != got[at:at + 3]
        results.append((name, wrong / total))
    return results


def as_array(screen):
    return np.frombuffer(screen, dtype=np.uint8).reshape(HEIGHT, WIDTH, 3)


_frames = {}


def spinning_logo(want, got):
    """Returns the fraction of the logo's pixels that are wrong for the nearest frame of
    its turn, as `spin_model` draws it with `spin.rs`'s own arithmetic, and whether that
    frame has it turned. Each screen should match one frame exactly."""
    _, left, top, width, height = REGIONS[0]
    key = bytes(want)
    if key not in _frames:
        _frames[key] = spin_model.all_frames(as_array(want)[top:top + height, left:left + width].copy())
    frames = _frames[key]
    extra = spin_model.EXTRA_ROWS
    drawn = as_array(got)[logo_reach()]
    # The square's circle clips the corners of this box, where the logo never reaches.
    square = square_pixels(drawn, top - extra, left)
    wrong_rows = {frame: (drawn != picture).any(axis=2) & ~square for frame, picture in frames.items()}
    wrong, frame = min((rows.mean(), frame) for frame, rows in wrong_rows.items())
    angle = spin_model.STEPS * frame // spin_model.TURN_FRAMES
    return wrong, frame, angle % spin_model.STEPS != 0


def compare_shapes(want, got):
    """Returns each region's name and fraction of pixels lit in one but dark in the other.
    The logo is left out, as it turns, and the capture's colors are too rough to tell
    which of its columns is which."""
    results = []
    for name, left, top, width, height in REGIONS[1:]:
        wrong = total = 0
        for y in range(top, top + height):
            for x in range(left, min(left + width, WIDTH - CAPTURE_X_SHIFT)):
                if in_orbit(x, y) or in_logo(x, y):
                    continue
                a = (y * WIDTH + x) * 3
                b = (y * WIDTH + x + CAPTURE_X_SHIFT) * 3
                total += 1
                wrong += (max(want[a:a + 3]) > LIT_LEVEL) != (max(got[b:b + 3]) > LIT_LEVEL)
        results.append((name, wrong / total))
    return results


def screens_from_qemu(image, key_presses):
    """Yields the screen at boot, then after each list of keys in `key_presses`."""
    with tempfile.TemporaryDirectory() as directory:
        monitor = f"{directory}/monitor"
        shot = f"{directory}/screen.png"
        qemu = subprocess.Popen(
            ["qemu-system-aarch64", "-M", "raspi4b", "-kernel", str(image), "-display", "none",
             "-monitor", f"unix:{monitor},server,nowait", "-serial", "stdio"],
            stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        try:
            time.sleep(BOOT_SECONDS)
            sock = socket.socket(socket.AF_UNIX)
            sock.connect(monitor)
            for keys in [[]] + key_presses:
                for key in keys:
                    qemu.stdin.write(key)
                    qemu.stdin.flush()
                    time.sleep(KEY_SECONDS)
                if qemu.poll() is not None:
                    raise SystemExit(f"qemu exited with {qemu.returncode}")
                sock.sendall(f"screendump {shot} -f png\n".encode())
                time.sleep(1)
                yield subprocess.run(["magick", shot, "-depth", "8", "rgb:-"], capture_output=True, check=True).stdout
        finally:
            qemu.kill()
            qemu.wait()


def screen_from_pi():
    with urllib.request.urlopen("http://localhost:8766/frame.jpg", timeout=5) as response:
        jpeg = response.read()
    return subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "mjpeg", "-i", "-", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        input=jpeg, capture_output=True, check=True,
    ).stdout


def aki_pictures(key):
    body, mouth = AKI_VERSIONS[key]
    # The original's mouth keeps no position of its own, as it isn't reduced.
    offset = (MOUTH_LEFT, MOUTH_TOP) if SLOTS[mouth][3] is None else (0, 0)
    return [(body, AKI_LEFT, AKI_TOP), (mouth, AKI_LEFT + offset[0], AKI_TOP + offset[1])]


def lit_fraction(screen, region):
    left, top, width, height = region
    lit = 0
    for y in range(top, top + height):
        row = screen[(y * WIDTH + left) * 3:(y * WIDTH + left + width) * 3]
        lit += sum(1 for at in range(0, len(row), 3) if row[at:at + 3] != b"\0\0\0")
    return lit / (width * height)


def square_colors_on(screen):
    """The rainbow's colors the square shows on a screen."""
    pixels = as_array(screen)
    found, counts = np.unique(pixels[square_pixels(pixels, 0, 0)], axis=0, return_counts=True)
    return {tuple(int(value) for value in color) for color, count in zip(found, counts) if count >= SQUARE_MIN_PIXELS}


def square_pixels(pixels, top, left):
    """Which of these pixels, whose top-left is at `left`, `top` on the screen, are the
    square's, on its circle."""
    ys, xs = np.mgrid[top:top + pixels.shape[0], left:left + pixels.shape[1]]
    distance_squared = (xs - ORBIT_CENTER[0]) ** 2 + (ys - ORBIT_CENTER[1]) ** 2
    on_circle = ((ORBIT_RADIUS - ORBIT_MARGIN) ** 2 <= distance_squared) & (distance_squared <= (ORBIT_RADIUS + ORBIT_MARGIN) ** 2)
    packed = pixels[..., 0].astype(np.int64) << 16 | pixels[..., 1].astype(np.int64) << 8 | pixels[..., 2]
    return on_circle & np.isin(packed, SQUARE_PACKED)


def logo_reach():
    """The logo's whole reach, as a slice of a screen array."""
    _, left, top, width, height = REGIONS[0]
    extra = spin_model.EXTRA_ROWS
    return slice(top - extra, top + height + extra), slice(left, left + width)


def lit_beside_cube(screen):
    """The fraction of pixels lit where the logo can reach, outside the cube's square,
    which should all be black while the logo steps aside."""
    _, left, top, width, height = REGIONS[0]
    extra = spin_model.EXTRA_ROWS
    cube_left, cube_top, cube_width, cube_height = CUBE_REGION
    drawn = as_array(screen)[logo_reach()]
    box = drawn.any(axis=2) & ~square_pixels(drawn, top - extra, left)
    box[cube_top - (top - extra):cube_top - (top - extra) + cube_height, cube_left - left:cube_left - left + cube_width] = False
    return box.mean()


def check_qemu(image):
    """Checks startup, a clear, then each version on a clear screen. The logo should
    be gone after any clear, and each version drawn over another is left out, as
    what that shows is simply one on top of the other."""
    to_aki = (CURSOR_OVER_AKI[0] - CURSOR_START[0], CURSOR_OVER_AKI[1] - CURSOR_START[1])
    away = (CURSOR_AWAY[0] - CURSOR_OVER_AKI[0], CURSOR_AWAY[1] - CURSOR_OVER_AKI[1])
    steps = [
        ("boot", [], STARTUP, None),
        ("a corrupted move does nothing", [corrupted(mouse_packet(*to_aki))], STARTUP, None),
        ("a move draws the cursor over Aki", [mouse_packet(*to_aki)], STARTUP, CURSOR_OVER_AKI),
        ("moving away restores what it covered", [mouse_packet(*away)], STARTUP, CURSOR_AWAY),
        ("a plain 0 is only text", [CLEAR_KEY], STARTUP, CURSOR_AWAY),
        ("a stray zero doesn't stop what follows", [b"\0", b"x"], STARTUP, CURSOR_AWAY),
        ("a corrupted key does nothing", [corrupted(key_packet(CLEAR_KEY))], STARTUP, CURSOR_AWAY),
        # The logo steps aside for the cube, so only Aki is expected around it.
        ("the cube spins", [key_packet(CUBE_KEY)], [picture for picture in STARTUP if picture != LOGO], CURSOR_AWAY),
        ("the cube stops", [key_packet(CUBE_KEY)], STARTUP, CURSOR_AWAY),
        # About a second apart, so the logo can't be resting in all of them.
        *[(f"the logo turns, {n}", [], STARTUP, CURSOR_AWAY) for n in range(1, 5)],
        # Clearing Aki's side for a new version must leave the turning logo alone.
        ("key 2 beside the logo", [key_packet(b"2")], [LOGO] + aki_pictures(b"2"), CURSOR_AWAY),
        ("a tone and a clip, then clear", [key_packet(TONE_KEY), key_packet(CLIP_KEY), key_packet(CLEAR_KEY)], [], None),
    ]
    for key in AKI_VERSIONS:
        steps.append((f"key {key.decode()} after a clear", [key_packet(CLEAR_KEY), key_packet(key)], aki_pictures(key), None))
    # x4's blocks reach past x1's edges, which must not be left around her.
    steps.append(("key 2 straight over 4", [key_packet(b"2")], aki_pictures(b"2"), None))
    good = True
    turned = 0
    square_colors = set()
    screens = screens_from_qemu(image, [keys for _, keys, _, _ in steps[1:]])
    for (label, _, pictures, tip), screen in zip(steps, screens):
        want = expected(pictures)
        if tip is not None:
            want = with_cursor(want, tip)
        for name, fraction in compare(want, screen):
            if name == "logo" and label == "the cube spins":
                fraction = lit_beside_cube(screen)
                print(f"{label}, {name}: {fraction:.4%} of the rest of its box is lit")
            elif name == "logo" and LOGO in pictures:
                fraction, frame, was_turned = spinning_logo(want, screen)
                turned += was_turned
                # Edge-on, the logo shows nothing, which an absent logo also matches. Just
                # after the cube, it should be back facing front, then turning.
                if label == "the cube stops" and not as_array(screen)[logo_reach()].any():
                    print(f"{label}, {name}: the logo didn't come back")
                    fraction = 1
                print(f"{label}, {name}: {fraction:.4%} of pixels wrong for frame {frame} of its turn, the nearest")
            else:
                print(f"{label}, {name}: {fraction:.4%} of pixels wrong")
            good = good and fraction == 0
        square_colors |= square_colors_on(screen)
        # Once it stops, the logo's exact match shows nothing of the cube was left.
        if label == "the cube spins":
            lit = lit_fraction(screen, CUBE_REGION)
            print(f"{label}: {lit:.1%} of its square is lit")
            good = good and lit >= CUBE_MIN_LIT
    # Seconds apart, it can't have been facing straight on in all of them.
    print(f"the logo was turned in {turned} screens")
    print(f"the square showed {len(square_colors)} of the rainbow's colors")
    return good and turned > 0 and len(square_colors) >= MIN_SQUARE_COLORS


def check_pi():
    results = compare_shapes(expected(STARTUP), screen_from_pi())
    for name, fraction in results:
        print(f"{name}: {fraction:.2%} of pixels don't match the expected shape")
    return all(fraction <= CAPTURE_MAX_WRONG for _, fraction in results)


if __name__ == "__main__":
    good = check_pi() if sys.argv[1:] == ["pi"] else check_qemu(NEXT / "target/kitsune-next.img")
    print("PASS" if good else "FAIL")
    sys.exit(0 if good else 1)
