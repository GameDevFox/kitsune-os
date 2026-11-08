#!/usr/bin/env python3
"""Measures JTAG write and read throughput to unused Pi RAM, checking the data."""
import hashlib
import os
import re
import socket
import subprocess
import sys
import time
from pathlib import Path

SP = sys.argv[1]
# This file is in next/tools/jtag/experiments.
NEXT = str(Path(__file__).resolve().parents[3])
TCL = ("127.0.0.1", 6666)
END = b"\x1a"
# Above the kernel image and stack, in plain RAM.
LOAD_ADDRESS = 0x01000000
SIZE = 256 * 1024
SPEEDS_KHZ = [1000, 2000, 4000, 8000, 12000, 20000]
COMMAND_TIMEOUT = 150

source = f"{SP}/speed-in.bin"
readback = f"{SP}/speed-out.bin"
with open(source, "wb") as out:
    out.write(os.urandom(SIZE))
want = hashlib.sha256(open(source, "rb").read()).hexdigest()


def command(sock, text):
    sock.sendall(text.encode() + END)
    data = b""
    while END not in data:
        chunk = sock.recv(65536)
        if not chunk:
            raise RuntimeError("openocd closed the connection")
        data += chunk
    return data.split(END)[0].decode(errors="replace").strip()


log = open(f"{SP}/openocd-speed.log", "w")
process = subprocess.Popen(["openocd", "-f", "tools/jtag/pi4.cfg"], cwd=NEXT, stdout=log, stderr=subprocess.STDOUT)
sock = None
try:
    deadline = time.monotonic() + 15
    while sock is None:
        if process.poll() is not None:
            raise RuntimeError(f"openocd exited with {process.returncode}")
        if time.monotonic() > deadline:
            raise RuntimeError("openocd never listened")
        try:
            sock = socket.create_connection(TCL, timeout=COMMAND_TIMEOUT)
        except OSError:
            time.sleep(0.2)

    print("state before:", command(sock, "bcm2711.cpu0 curstate"))
    command(sock, "targets bcm2711.cpu0")
    print("halt:", command(sock, "halt 2000") or "ok")
    print("state:", command(sock, "bcm2711.cpu0 curstate"))
    print("mode:", (re.findall(r"cpsr[^\n]*", command(sock, "reg cpsr")) or ["?"])[0][:60])

    print(f"\n{'clock':>9} {'write':>12} {'read':>12}  data")
    for speed in SPEEDS_KHZ:
        command(sock, f"adapter speed {speed}")
        start = time.monotonic()
        result = command(sock, f"load_image {source} {LOAD_ADDRESS:#x} bin")
        write_seconds = time.monotonic() - start
        if "downloaded" not in result:
            print(f"{speed:>6}kHz  write failed: {result[:100]}")
            break

        if os.path.exists(readback):
            os.remove(readback)
        start = time.monotonic()
        result = command(sock, f"dump_image {readback} {LOAD_ADDRESS:#x} {SIZE}")
        read_seconds = time.monotonic() - start
        got = hashlib.sha256(open(readback, "rb").read()).hexdigest() if os.path.exists(readback) else "none"
        ok = got == want
        print(f"{speed:>6}kHz {SIZE / 1024 / write_seconds:>7.1f} KiB/s {SIZE / 1024 / read_seconds:>7.1f} KiB/s  {'match' if ok else 'MISMATCH'}", flush=True)
        if not ok:
            print(f"  stopped: {result[:100]}")
            break
finally:
    if sock is not None:
        try:
            command(sock, "adapter speed 1000")
            if command(sock, "bcm2711.cpu0 curstate") == "halted":
                print("\nresume:", command(sock, "targets bcm2711.cpu0; resume") or "ok")
            print("state after:", command(sock, "bcm2711.cpu0 curstate"))
            sock.sendall(b"shutdown" + END)
        except (OSError, RuntimeError) as error:
            print(f"cleanup problem: {error}")
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
    print(f"openocd rc={process.returncode}")
