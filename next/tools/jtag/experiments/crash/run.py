#!/usr/bin/env python3
"""Finds out whether JTAG causes the kernel's crash after a watchdog reboot.

Reboots the card's kernel repeatedly, then either leaves the core alone, repeats
what reset.py does in the window just after a reboot, or halts it much later. Watches
serial for a crash each time. Only reads memory, apart from the watchdog registers.
Puts the current kernel back at the end.
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
TEST_ADDRESS = 0xFE003004
ROUNDS = 4
# reset.py waits this long after the reset before it starts halting.
RESET_SCRIPT_DELAY = 3
RECOVERY_TIMEOUT = 40
LATE_HALT_AT = 25
WATCH_UNTIL = 35
READY_TEXT = b"Keys:"

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


def open_viewer():
    viewer = socket.create_connection(SERIAL, timeout=5)
    viewer.settimeout(0.2)
    header = b""
    while not header.endswith(b"\n"):
        header += viewer.recv(1)
    left = int(header.split(b"=")[1])
    while left > 0:
        left -= len(viewer.recv(min(left, 65536)))
    return viewer


def reboot():
    command("targets bcm2711.cpu0")
    command("halt 2000")
    if command("bcm2711.cpu0 curstate") != "halted":
        sys.exit("core 0 did not halt, so the watchdog can't be reached")
    command(f"mww {PM_WDOG:#x} {PM_PASSWORD | WDOG_TICKS:#x}")
    start = time.monotonic()
    command(f"mww {PM_RSTC:#x} {PM_PASSWORD | FULL_RESET:#x}")
    return start


def like_reset_script(start):
    """What reset.py does: halt and read until a read works, then resume."""
    time.sleep(RESET_SCRIPT_DELAY)
    while time.monotonic() - start < RECOVERY_TIMEOUT:
        command("targets bcm2711.cpu0")
        command("halt 2000")
        if ":" in command(f"mdw {TEST_ADDRESS:#x} 1"):
            command("resume")
            return time.monotonic() - start
        time.sleep(1)
    sys.exit("JTAG did not come back")


def late_halt(start):
    time.sleep(max(0, LATE_HALT_AT - (time.monotonic() - start)))
    command("targets bcm2711.cpu0")
    command("halt 2000")
    command(f"mdw {TEST_ADDRESS:#x} 1")
    command("resume")
    return time.monotonic() - start


def watch(viewer, start, serial):
    while time.monotonic() - start < WATCH_UNTIL:
        try:
            serial += viewer.recv(65536)
        except TimeoutError:
            pass
    return serial


def run_round(viewer, mode):
    start = reboot()
    resumed = None
    if mode == "reset script":
        resumed = like_reset_script(start)
    elif mode == "late halt":
        resumed = late_halt(start)
    serial = watch(viewer, start, b"")
    text = serial.decode(errors="replace")
    fault = re.search(r"at (0x[0-9a-f]+) accessing (0x[0-9a-f]+)", text)
    pitch = re.search(r"x22 (0x[0-9a-f]+)", text)
    booted = READY_TEXT in serial
    at = f", resumed at {resumed:.1f}s" if resumed else ""
    kind = re.search(r"FATAL EXCEPTION: (.*)", text)
    where = f" at {fault[1]} accessing {fault[2]}" if fault else ""
    outcome = f"CRASH: {kind[1].strip()}{where}, x22 {pitch[1] if pitch else '?'}" if kind else "fine"
    print(f"{mode:12} booted={booted}{at}: {outcome}", flush=True)
    return "FATAL" in text


viewer = open_viewer()
crashes = {}
for round_number in range(ROUNDS):
    for mode in ("hands off", "reset script", "late halt"):
        crashes[mode] = crashes.get(mode, 0) + run_round(viewer, mode)
for mode, count in crashes.items():
    print(f"{mode}: {count} of {ROUNDS} crashed", flush=True)

restore = subprocess.run(["cargo", "pi"], cwd=SRC, capture_output=True, text=True)
print("restored:", (restore.stdout + restore.stderr).strip().splitlines()[-1], "exit", restore.returncode, flush=True)
sys.exit(restore.returncode)
