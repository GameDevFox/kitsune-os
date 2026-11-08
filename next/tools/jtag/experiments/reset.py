#!/usr/bin/env python3
"""Reboots the Pi through its watchdog over JTAG, and waits until JTAG works again."""
import socket
import sys
import time

TCL = ("127.0.0.1", 6666)
PM_RSTC, PM_WDOG = 0xFE10001C, 0xFE100024
PM_PASSWORD, FULL_RESET, WDOG_TICKS = 0x5A000000, 0x20, 10
TEST_ADDRESS = 0xFE003004
RECOVERY_TIMEOUT = 40

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


command("targets bcm2711.cpu0")
command("halt 2000")
if command("bcm2711.cpu0 curstate") != "halted":
    sys.exit("core 0 did not halt, so the watchdog can't be reached")
command(f"mww {PM_WDOG:#x} {PM_PASSWORD | WDOG_TICKS:#x}")
start = time.monotonic()
command(f"mww {PM_RSTC:#x} {PM_PASSWORD | FULL_RESET:#x}")
time.sleep(3)

# For several seconds after a reset, memory access over JTAG returns nothing.
while time.monotonic() - start < RECOVERY_TIMEOUT:
    command("targets bcm2711.cpu0")
    command("halt 2000")
    reply = command(f"mdw {TEST_ADDRESS:#x} 1")
    if ":" in reply:
        command("resume")
        print(f"JTAG back {time.monotonic() - start:.1f}s after the reset; timer {reply.split(':')[1].strip()}")
        sys.exit(0)
    time.sleep(1)
sys.exit("JTAG did not come back")
