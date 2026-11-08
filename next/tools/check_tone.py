#!/usr/bin/env python3
"""Presses the kernel's tone key through the server, records this PC's line-in, and
checks that each side of the headphone jack carried its pitch.

Usage: check_tone.py [SERVER]

The pitches must match `TEST_TONE` in `kernel/src/sound.rs`. Needs `cargo server`
running, the jack wired to the line-in that `line_in.py` names, and the Pi idle, as
the key goes to whatever kernel is running.
"""
import sys
import time
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import line_in

SERVER = sys.argv[1] if len(sys.argv) > 1 else "localhost:8766"
TONE_KEY = b"7"
# Middle C on the right, and the G above it on the left.
EXPECTED = {"left": 392.00, "right": 261.63}
# How far below its note each side's third harmonic must be, for a sine.
MIN_PURITY_DB = 30
# Time for the recording to start before the key, and for the 0.5 s tone after it.
LEAD_IN_SECONDS = 0.5
RECORD_SECONDS = 2
# Each side's level, compared with the line's noise floor, must show the tone happened.
MIN_TONE_RMS = 1000


def press():
    time.sleep(LEAD_IN_SECONDS)
    request = urllib.request.Request(f"http://{SERVER}/key", data=TONE_KEY, method="POST")
    with urllib.request.urlopen(request) as response:
        response.read()


def main():
    samples = line_in.record_during(RECORD_SECONDS, press)
    failures = line_in.check_pitches(samples, EXPECTED)
    failures += line_in.check_purity(samples, EXPECTED, MIN_PURITY_DB)
    for channel, name in enumerate(line_in.SIDES):
        rms = float((samples[:, channel] - samples[:, channel].mean()).std())
        loud = rms >= MIN_TONE_RMS
        failures += not loud
        print(f"{name}: level {rms:.0f} of 32767 RMS: {'ok' if loud else 'FAIL, too quiet'}")
    sys.exit(1 if failures else 0)


main()
