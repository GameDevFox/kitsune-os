#!/usr/bin/env python3
"""Plants values in RAM and registers, then shows which survive a reboot.

  reset-survival.py STATE_FILE plant    Plant, read back as a control
  reset-survival.py STATE_FILE watchdog Fire the watchdog, wait for boot
  reset-survival.py STATE_FILE check    Compare, once the Pi has rebooted
"""
import json
import os
import re
import socket
import sys
import time

STATE, MODE = sys.argv[1], sys.argv[2]
TCL = ("127.0.0.1", 6666)
VIEWER = ("localhost", 8765)
END = b"\x1a"
WORDS = 16
BOOT_TIMEOUT = 40
RETRY_SECONDS = 30

# Well above the kernel and below the GPU's memory, which starts near 0x3b400000.
RAM = [0x00200000, 0x01000000, 0x08000000, 0x10000000, 0x20000000, 0x30000000]

PM_RSTC, PM_RSTS, PM_WDOG = 0xFE10001C, 0xFE100020, 0xFE100024
PM_PASSWORD, FULL_RESET, WDOG_TICKS = 0x5A000000, 0x20, 10
TIMER_LOW = 0xFE003004

# Registers to plant in, with values they accept. Nothing here is enabled, so
# they have no effect while IRQs stay masked.
PLANTED = {
    "system timer compare 1": (0xFE003010, 0x12345678),
    "system timer compare 3": (0xFE003018, 0x9ABCDEF0),
    "ARM timer load": (0xFE00B400, 0x00C0FFEE),
    "ARM timer reload": (0xFE00B418, 0x00BEEF00),
}
# Observed only, to see what the firmware leaves behind.
OBSERVED = {
    "reset status PM_RSTS": PM_RSTS,
    "UART baud integer": 0xFE201024,
    "UART baud fraction": 0xFE201028,
    "UART line control": 0xFE20102C,
    "UART control": 0xFE201030,
    "GPIO function 10-19 (UART)": 0xFE200004,
    "GPIO function 20-29 (JTAG)": 0xFE200008,
}

sock = socket.create_connection(TCL, timeout=20)


def command(text):
    sock.sendall(text.encode() + END)
    data = b""
    while END not in data:
        chunk = sock.recv(65536)
        if not chunk:
            raise RuntimeError("openocd closed the connection")
        data += chunk
    return data.split(END)[0].decode(errors="replace").strip()


def read_words(address, count=1):
    reply = command(f"mdw {address:#x} {count}")
    words = []
    for line in reply.splitlines():
        match = re.match(r"0x[0-9a-f]+:\s+(.*)", line.strip())
        if match:
            words += [int(word, 16) for word in match.group(1).split()]
    if len(words) != count:
        raise RuntimeError(f"read of {address:#x} gave {reply[:80]!r}")
    return words


def halt():
    command("targets bcm2711.cpu0")
    command("halt 2000")
    state = command("bcm2711.cpu0 curstate")
    if state != "halted":
        raise RuntimeError(f"core 0 did not halt: {state}")


def snapshot():
    return {name: read_words(address)[0] for name, address in OBSERVED.items()}


def plant():
    halt()
    state = {"ram": {}, "planted_before": {}, "timer_before": read_words(TIMER_LOW)[0]}
    for address in RAM:
        pattern = [int.from_bytes(os.urandom(4), "little") for _ in range(WORDS)]
        for index, word in enumerate(pattern):
            command(f"mww {address + index * 4:#x} {word:#x}")
        state["ram"][hex(address)] = pattern
    for name, (address, value) in PLANTED.items():
        command(f"mww {address:#x} {value:#x}")
        state["planted_before"][name] = read_words(address)[0]
    state["observed_before"] = snapshot()

    print("control, read straight back with no reset:")
    bad = 0
    for address, pattern in state["ram"].items():
        same = sum(a == b for a, b in zip(read_words(int(address, 16), WORDS), pattern))
        bad += same != WORDS
        print(f"  RAM {address:>10}: {same}/{WORDS} words match")
    for name, (address, value) in PLANTED.items():
        got = state["planted_before"][name]
        print(f"  {name:24} wrote {value:#010x}, reads {got:#010x}")
    print(f"timer before reset: {state['timer_before']} us ({state['timer_before'] / 1e6:.1f} s)")
    json.dump(state, open(STATE, "w"))
    if bad:
        raise RuntimeError("the control failed: RAM didn't hold the pattern")


def watchdog():
    viewer = socket.create_connection(VIEWER)
    viewer.settimeout(0.1)
    header = b""
    while not header.endswith(b"\n"):
        header += viewer.recv(1)
    left = int(header.split(b"=")[1])
    while left > 0:
        left -= len(viewer.recv(min(left, 65536)))

    halt()
    command(f"mww {PM_WDOG:#x} {PM_PASSWORD | WDOG_TICKS:#x}")
    start = time.monotonic()
    command(f"mww {PM_RSTC:#x} {PM_PASSWORD | FULL_RESET:#x}")

    data = b""
    while time.monotonic() - start < BOOT_TIMEOUT:
        try:
            data += viewer.recv(4096)
        except TimeoutError:
            pass
        if b"x30" in data:
            print(f"card kernel finished printing {time.monotonic() - start:.1f}s after the reset")
            return
    raise RuntimeError(f"no boot seen: {data[-200:]!r}")


def check():
    before = json.load(open(STATE))
    deadline = time.monotonic() + RETRY_SECONDS
    while True:
        try:
            halt()
            timer_after = read_words(TIMER_LOW)[0]
            break
        except RuntimeError as error:
            if time.monotonic() > deadline:
                raise
            print(f"  waiting for JTAG: {str(error)[:70]}")
            time.sleep(2)

    print(f"timer: {before['timer_before'] / 1e6:.1f} s before, {timer_after / 1e6:.1f} s now "
          f"({'RESTARTED' if timer_after < before['timer_before'] else 'NOT restarted'})")

    print("\nRAM, by how many words kept their value:")
    for address, pattern in before["ram"].items():
        got = read_words(int(address, 16), WORDS)
        same = sum(a == b for a, b in zip(got, pattern))
        print(f"  {address:>10}: {same}/{WORDS}  ({'survived' if same == WORDS else 'partly' if same else 'lost'})  now starts {got[0]:#010x}")

    print("\nplanted registers:")
    for name, (address, value) in PLANTED.items():
        got = read_words(address)[0]
        print(f"  {name:24} planted {value:#010x}, now {got:#010x}  ({'SURVIVED' if got == value else 'reset'})")

    print("\nobserved registers, before and after:")
    after = snapshot()
    for name in OBSERVED:
        a, b = before["observed_before"][name], after[name]
        print(f"  {name:28} {a:#010x} -> {b:#010x}{'' if a == b else '  CHANGED'}")
    command("resume")


{"plant": plant, "watchdog": watchdog, "check": check}[MODE]()
