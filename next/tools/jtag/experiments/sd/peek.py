#!/usr/bin/env python3
"""Shows the state the firmware left the SD controller (EMMC2) in. Changes nothing.

Reads 32 bits at a time, since Linux's driver only uses 32-bit access on this chip.
Skips the data port, as reading that consumes data.
"""
import socket
import sys

TCL = ("127.0.0.1", 6666)
EMMC2 = 0xFE340000
DATA_PORT = 0x20
# Names from the SDHCI spec, at their offsets. Some words hold several registers.
REGISTERS = {
    0x00: "SDMA address",
    0x04: "block count << 16 | block size",
    0x08: "argument",
    0x0C: "command << 16 | transfer mode",
    0x10: "response 0",
    0x14: "response 1",
    0x18: "response 2",
    0x1C: "response 3",
    0x24: "present state",
    0x28: "wakeup | block gap | power | host control",
    0x2C: "reset | timeout | clock",
    0x30: "interrupt status",
    0x34: "interrupt status enable",
    0x38: "interrupt signal enable",
    0x3C: "host control 2 | auto CMD error",
    0x40: "capabilities 0",
    0x44: "capabilities 1",
    0xFC: "version | slot interrupts",
}
PRESENT_BITS = {
    0: "cmd inhibit", 1: "dat inhibit", 2: "dat active", 8: "write active", 9: "read active",
    10: "buffer write ready", 11: "buffer read ready", 16: "card inserted", 17: "card stable",
}

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
was_running = command("bcm2711.cpu0 curstate") == "running"
command("halt 2000")
if command("bcm2711.cpu0 curstate") != "halted":
    sys.exit("core 0 did not halt")
try:
    for offset, name in REGISTERS.items():
        assert offset != DATA_PORT
        reply = command(f"mdw {EMMC2 + offset:#x} 1")
        value = reply.split(":")[1].strip() if ":" in reply else f"<{reply}>"
        print(f"{offset:#04x} {value:>10}  {name}")
        if offset == 0x24 and ":" in reply:
            state = int(value, 16)
            print("     " + ", ".join(label for bit, label in PRESENT_BITS.items() if state >> bit & 1))
finally:
    if was_running:
        command("resume")
print("core 0:", command("bcm2711.cpu0 curstate"))
