#!/usr/bin/env python3
"""Times a watchdog reboot, until the card's kernel first prints, with hands off JTAG.

Serial is read through the server, so this is good to a few tens of milliseconds.
Puts the current kernel back at the end, as `cargo pi` would have left it.
"""
import re
import socket
import subprocess
import sys
import time
from pathlib import Path

SRC = Path(__file__).resolve().parents[3]
TCL = ("127.0.0.1", 6666)
SERIAL = ("localhost", 8765)
PM_RSTC, PM_WDOG = 0xFE10001C, 0xFE100024
PM_PASSWORD, FULL_RESET, WDOG_TICKS = 0x5A000000, 0x20, 10
ROUNDS = 3
FIRST_TEXT = b"Hello"
LAST_TEXT = b"Keys:"
TIMEOUT = 30
# Long enough for JTAG to work again, as the next round has to halt the core.
SETTLE_SECONDS = 20

tcl = socket.create_connection(TCL, timeout=20)


def command(text):
    tcl.sendall(text.encode() + b"\x1a")
    data = b""
    while b"\x1a" not in data:
        chunk = tcl.recv(65536)
        if not chunk:
            sys.exit("openocd closed the connection")
        data += chunk
    return data.split(b"\x1a")[0].decode(errors="replace").strip()


viewer = socket.create_connection(SERIAL, timeout=5)
viewer.settimeout(0.01)
header = b""
while not header.endswith(b"\n"):
    try:
        header += viewer.recv(1)
    except TimeoutError:
        pass
left = int(header.split(b"=")[1])
while left > 0:
    try:
        left -= len(viewer.recv(min(left, 65536)))
    except TimeoutError:
        pass

for number in range(ROUNDS):
    command("targets bcm2711.cpu0")
    command("halt 2000")
    if command("bcm2711.cpu0 curstate") != "halted":
        sys.exit("core 0 did not halt")
    command(f"mww {PM_WDOG:#x} {PM_PASSWORD | WDOG_TICKS:#x}")
    start = time.monotonic()
    command(f"mww {PM_RSTC:#x} {PM_PASSWORD | FULL_RESET:#x}")
    first = last = None
    serial = b""
    while time.monotonic() - start < TIMEOUT and last is None:
        try:
            serial += viewer.recv(65536)
        except TimeoutError:
            pass
        now = time.monotonic() - start
        if first is None and FIRST_TEXT in serial:
            first = now
        if LAST_TEXT in serial:
            last = now
    text = serial.decode(errors="replace")
    drawn = re.search(r"Images drawn in \d+ ms", text)
    print(f"round {number + 1}: first text at {first if first is None else round(first, 2)} s, ready at {last if last is None else round(last, 2)} s, {drawn[0] if drawn else 'no images line'}", flush=True)
    time.sleep(max(0, SETTLE_SECONDS - (time.monotonic() - start)))

restore = subprocess.run(["cargo", "pi"], cwd=SRC, capture_output=True, text=True)
print("restored:", (restore.stdout + restore.stderr).strip().splitlines()[-1], "exit", restore.returncode, flush=True)
