#!/usr/bin/env python3
"""Presses the kernel's clip key through the server, records this PC's line-in, and
checks that each side of the headphone jack played the clip `voice.py` last loaded.

Usage: check_voice.py [SERVER]

Each side's recording is lined up with the clip, and their correlation measured in
the speech band: 1 would be identical. Playing it at the wrong speed, or garbled, or
not at all, gives far less. It also watches the screen while the clip plays, as sound
plays in the background and the orbiting square should keep moving. Needs `cargo
server` running, the jack wired to the line-in, and the HDMI on the capture card.
"""
import subprocess
import struct
import sys
import time
import urllib.request
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import line_in

SERVER = sys.argv[1] if len(sys.argv) > 1 else "localhost:8766"
CLIP_KEY = b"8"
CLIP = Path(__file__).resolve().parent.parent / "target/voice.bin"
MAGIC = b"KITSND01"
HEADER = struct.Struct("<8sII")
# Time for the recording to start before the key, and for the ramps and the line-in's
# settling after the clip.
LEAD_IN_SECONDS = 0.5
TAIL_SECONDS = 1.0
# Lined up, each side must match the clip at least this well, in the band where speech
# is. Below it, the pop as the PWM starts, and the line-in settling after it, dominate,
# and above it there's little of a voice.
MIN_CORRELATION = 0.9
SPEECH_BAND_HZ = (300, 3000)

SCREEN_WIDTH, SCREEN_HEIGHT = 1920, 1080
# The square's orbit, from `square.rs`: around the logo's center with a radius of 190,
# and 40 pixels across, with some margin. Only the ring it travels is searched, which
# leaves out the turning logo inside it.
ORBIT_CENTER = (1250, 272)
ORBIT_RADIUS = 190
ORBIT_MARGIN = 32
# The square cycles through colors, so it's found by what changed in the ring since the
# last frame: where it was and where it is. A change must be more than the capture's
# noise, summed over red, green and blue, and cover a good part of a square.
CHANGE_MIN_LEVEL = 90
SQUARE_MIN_PIXELS = 400
# Positions are compared in steps this many pixels wide, ignoring the capture's noise.
POSITION_STEP = 5
FRAME_INTERVAL_SECONDS = 0.2
FRAME_TIMEOUT_SECONDS = 2
# The square moves about 190 pixels a second, so a clip of over a second sees it in
# at least this many places, unless drawing stops.
MIN_POSITIONS = 3


def clip_samples():
    data = CLIP.read_bytes()
    magic, rate, count = HEADER.unpack_from(data)
    if magic != MAGIC:
        sys.exit(f"{CLIP} isn't a clip; run voice.py first")
    samples = np.frombuffer(data, dtype="<i2", count=count, offset=HEADER.size).astype(np.float64)
    # The PWM plays at its own rate, slightly off the line-in's, so resample to match.
    times = np.arange(int(count * line_in.SAMPLE_RATE / rate)) / line_in.SAMPLE_RATE
    return np.interp(times, np.arange(count) / rate, samples)


def screen():
    with urllib.request.urlopen(f"http://{SERVER}/frame.jpg", timeout=FRAME_TIMEOUT_SECONDS) as response:
        jpeg = response.read()
    raw = subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "mjpeg", "-i", "-", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        input=jpeg, capture_output=True, check=True,
    ).stdout
    return np.frombuffer(raw, dtype=np.uint8).reshape(SCREEN_HEIGHT, SCREEN_WIDTH, 3).astype(int)


def ring():
    ys, xs = np.mgrid[0:SCREEN_HEIGHT, 0:SCREEN_WIDTH]
    distance_squared = (xs - ORBIT_CENTER[0]) ** 2 + (ys - ORBIT_CENTER[1]) ** 2
    return ((ORBIT_RADIUS - ORBIT_MARGIN) ** 2 <= distance_squared) & (distance_squared <= (ORBIT_RADIUS + ORBIT_MARGIN) ** 2)


def square_position(previous, current, in_ring):
    """The middle of what changed in the ring between two screens, or None if too little
    did: the square was there, and is there now."""
    changed = np.abs(current - previous).sum(axis=2) > CHANGE_MIN_LEVEL
    ys, xs = np.nonzero(in_ring & changed)
    if len(xs) < SQUARE_MIN_PIXELS:
        return None
    return round(xs.mean() / POSITION_STEP), round(ys.mean() / POSITION_STEP)


def press(clip_seconds):
    """Presses the key, then notes where the square is while the clip plays."""
    time.sleep(LEAD_IN_SECONDS)
    request = urllib.request.Request(f"http://{SERVER}/key", data=CLIP_KEY, method="POST")
    with urllib.request.urlopen(request) as response:
        response.read()
    positions = []
    in_ring = ring()
    previous = screen()
    started = time.monotonic()
    while time.monotonic() - started < clip_seconds:
        time.sleep(FRAME_INTERVAL_SECONDS)
        current = screen()
        positions.append(square_position(previous, current, in_ring))
        previous = current
    return positions


def speech(samples):
    """Keeps only the speech band."""
    spectrum = np.fft.rfft(samples)
    frequencies = np.fft.rfftfreq(len(samples), 1 / line_in.SAMPLE_RATE)
    low, high = SPEECH_BAND_HZ
    spectrum[(frequencies < low) | (frequencies > high)] = 0
    return np.fft.irfft(spectrum, len(samples))


def lined_up_correlation(recorded, clip):
    """Finds where the clip best fits in the recording, and how well it fits there."""
    size = 1 << int(np.ceil(np.log2(len(recorded) + len(clip))))
    scores = np.fft.irfft(np.fft.rfft(recorded, size) * np.conj(np.fft.rfft(clip, size)), size)
    lag = int(np.argmax(scores[: len(recorded) - len(clip) + 1]))
    part = recorded[lag:lag + len(clip)]
    part, reference = part - part.mean(), clip - clip.mean()
    return lag, float(np.dot(part, reference) / np.sqrt(np.dot(part, part) * np.dot(reference, reference)))


def main():
    clip = clip_samples()
    seconds = LEAD_IN_SECONDS + len(clip) / line_in.SAMPLE_RATE + TAIL_SECONDS
    clip_seconds = len(clip) / line_in.SAMPLE_RATE
    positions = []
    recorded = line_in.record_during(seconds, lambda: positions.extend(press(clip_seconds)))
    failures = 0
    for channel, name in enumerate(line_in.SIDES):
        lag, correlation = lined_up_correlation(speech(recorded[:, channel]), speech(clip))
        good = correlation >= MIN_CORRELATION
        failures += not good
        print(f"{name}: starts {lag / line_in.SAMPLE_RATE:.2f} s in, correlation {correlation:.3f}: {'ok' if good else 'FAIL'}")
    seen = [position for position in positions if position is not None]
    places = len(set(seen))
    moving = places >= MIN_POSITIONS
    failures += not moving
    print(f"screen: the square was in {places} places in {len(seen)} of {len(positions)} frames while it played: {'ok' if moving else 'FAIL'}")
    sys.exit(1 if failures else 0)


main()
