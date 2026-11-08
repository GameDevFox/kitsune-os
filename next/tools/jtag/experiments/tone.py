#!/usr/bin/env python3
"""Plays a square wave on each channel of the Pi's headphone jack, records this PC's
line-in, and checks that each channel carries its own pitch, then turns it off.

Usage: tone.py [SECONDS]

The PWM makes the wave by itself, so this needs no kernel support: it only writes the
clock manager and PWM1 over JTAG, briefly halting core 0 each time. The firmware
already gives GPIO 40 and 41 to PWM1, which this checks rather than changes. Needs
the jack wired to the line-in that `tools/line_in.py` names, and `parecord` and numpy on this PC.
"""
import socket
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
import line_in

TCL = ("127.0.0.1", 6666)
SECONDS = float(sys.argv[1]) if len(sys.argv) > 1 else 3

GPFSEL4 = 0xFE200010
# Three bits per pin, starting from GPIO 40, and 0b100 selects alternate function 0.
GPIO_40_41_MASK, GPIO_40_41_ALT0 = 0o77, 0o44

CM_PWMCTL, CM_PWMDIV = 0xFE1010A0, 0xFE1010A4
CM_PASSWORD = 0x5A000000
CM_SOURCE_OSCILLATOR, CM_ENABLE, CM_BUSY = 0x1, 0x10, 0x80
CM_DIVISOR_SHIFT = 12
OSCILLATOR_HZ = 54_000_000
CLOCK_DIVISOR = 1000
PWM_CLOCK_HZ = OSCILLATOR_HZ // CLOCK_DIVISOR

PWM1 = 0xFE20C800
CTL, RNG1, DAT1, RNG2, DAT2 = 0x00, 0x10, 0x14, 0x20, 0x24
# Enable each channel in mark-space mode, which outputs a plain high-then-low pulse.
PWEN1, MSEN1, PWEN2, MSEN2 = 0x1, 0x80, 0x100, 0x8000

# Pulses per wave, which sets each channel's pitch: about 439 Hz on the left and 659 Hz
# on the right, which aren't harmonics of each other.
LEFT_RANGE, RIGHT_RANGE = 123, 82

sock = socket.create_connection(TCL, timeout=20)


def command(text):
    sock.sendall(text.encode() + b"\x1a")
    data = b""
    while b"\x1a" not in data:
        chunk = sock.recv(65536)
        if not chunk:
            sys.exit("openocd closed the connection")
        data += chunk
    return data.split(b"\x1a")[0].decode(errors="replace").strip()


def write(address, value):
    command(f"mww {address:#x} {value:#x}")


def read(address):
    return int(command(f"read_memory {address:#x} 32 1"), 16)


def halted(action):
    """Runs `action` with core 0 halted, as memory access goes through it."""
    command("targets bcm2711.cpu0")
    command("halt 2000")
    if command("bcm2711.cpu0 curstate") != "halted":
        sys.exit("core 0 did not halt")
    try:
        return action()
    finally:
        command("resume")


def start():
    if read(GPFSEL4) & GPIO_40_41_MASK != GPIO_40_41_ALT0:
        sys.exit(f"GPIO 40 and 41 aren't set to PWM: GPFSEL4 is {read(GPFSEL4):#x}")
    # The clock's divisor may only change while it's stopped and no longer busy.
    write(CM_PWMCTL, CM_PASSWORD | CM_SOURCE_OSCILLATOR)
    if read(CM_PWMCTL) & CM_BUSY:
        sys.exit("the PWM clock didn't stop")
    write(CM_PWMDIV, CM_PASSWORD | CLOCK_DIVISOR << CM_DIVISOR_SHIFT)
    write(CM_PWMCTL, CM_PASSWORD | CM_SOURCE_OSCILLATOR | CM_ENABLE)
    # Channel 1, on GPIO 40, is the jack's right side, as recorded on 2026-11-07.
    for range_at, data_at, pulses in ((RNG1, DAT1, RIGHT_RANGE), (RNG2, DAT2, LEFT_RANGE)):
        write(PWM1 + range_at, pulses)
        write(PWM1 + data_at, pulses // 2)
    write(PWM1 + CTL, PWEN1 | MSEN1 | PWEN2 | MSEN2)


def stop():
    write(PWM1 + CTL, 0)
    write(CM_PWMCTL, CM_PASSWORD | CM_SOURCE_OSCILLATOR)


def main():
    expected = {"left": PWM_CLOCK_HZ / LEFT_RANGE, "right": PWM_CLOCK_HZ / RIGHT_RANGE}
    try:
        samples = line_in.record_during(SECONDS, lambda: halted(start))
    finally:
        halted(stop)
    sys.exit(1 if line_in.check_pitches(samples, expected) else 0)


main()
