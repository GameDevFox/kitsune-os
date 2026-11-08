#!/usr/bin/env python3
"""Asks the firmware how RAM is split, then paints into the framebuffer over JTAG.

Usage: framebuffer.py SCRATCH_DIR
"""
import socket
import struct
import sys
import time
from pathlib import Path

SCRATCH = Path(sys.argv[1])
TCL = ("127.0.0.1", 6666)
END = b"\x1a"

MAILBOX_READ = 0xFE00B880
MAILBOX_STATUS = 0xFE00B898
MAILBOX_WRITE = 0xFE00B8A0
MAILBOX_FULL = 1 << 31
MAILBOX_EMPTY = 1 << 30
PROPERTY_CHANNEL = 8
RESPONSE_OK = 0x80000000
# Unused RAM for the message, which must be 16-byte aligned.
MESSAGE_ADDRESS = 0x00200000

# Read-only queries: tag, and how many words of value it returns.
TAGS = {
    "board revision": (0x00010002, 1),
    "ARM memory": (0x00010005, 2),
    "GPU memory": (0x00010006, 2),
    "physical display size": (0x00040003, 2),
    "bits per pixel": (0x00040005, 1),
}

FRAMEBUFFER = 0x3E3CF000
WIDTH = 1920
BYTES_PER_PIXEL = 4
STRIPE_ROWS = 60
STRIPE_GAP_ROWS = 20
STRIPE_START_ROW = 300
PAUSE_SECONDS = 3
# Stored as blue, green, red, alpha.
COLORS = {"red": (0x00, 0x00, 0xFF, 0xFF), "green": (0x00, 0xFF, 0x00, 0xFF), "blue": (0xFF, 0x00, 0x00, 0xFF)}

sock = socket.create_connection(TCL, timeout=60)


def command(text):
    sock.sendall(text.encode() + END)
    data = b""
    while END not in data:
        chunk = sock.recv(65536)
        if not chunk:
            raise RuntimeError("openocd closed the connection")
        data += chunk
    return data.split(END)[0].decode(errors="replace").strip()


def read_words(address, count):
    reply = command(f"mdw {address:#x} {count}")
    words = []
    for line in reply.splitlines():
        if ":" in line:
            words += [int(word, 16) for word in line.split(":", 1)[1].split()]
    if len(words) != count:
        raise RuntimeError(f"read of {address:#x} gave {reply[:90]!r}")
    return words


def halt():
    command("targets bcm2711.cpu0")
    command("halt 2000")
    state = command("bcm2711.cpu0 curstate")
    if state != "halted":
        raise RuntimeError(f"core 0 did not halt: {state}")


def build_message():
    """Returns the message's words, and where each tag's value lands in it."""
    words = [0, 0]
    positions = {}
    for name, (tag, value_words) in TAGS.items():
        words += [tag, value_words * 4, 0]
        positions[name] = (len(words), value_words)
        words += [0] * value_words
    words.append(0)
    words[0] = len(words) * 4
    return words, positions


def ask_firmware():
    words, positions = build_message()
    for index, word in enumerate(words):
        command(f"mww {MESSAGE_ADDRESS + index * 4:#x} {word:#x}")

    if read_words(MAILBOX_STATUS, 1)[0] & MAILBOX_FULL:
        raise RuntimeError("the mailbox is full")
    command(f"mww {MAILBOX_WRITE:#x} {MESSAGE_ADDRESS | PROPERTY_CHANNEL:#x}")

    deadline = time.monotonic() + 3
    while read_words(MESSAGE_ADDRESS + 4, 1)[0] != RESPONSE_OK:
        if time.monotonic() > deadline:
            raise RuntimeError(f"no answer: {read_words(MESSAGE_ADDRESS, 3)}")
        time.sleep(0.05)
    # Take the reply off the queue, so the kernel never finds a stale one.
    while not read_words(MAILBOX_STATUS, 1)[0] & MAILBOX_EMPTY:
        read_words(MAILBOX_READ, 1)

    reply = read_words(MESSAGE_ADDRESS, len(words))
    return {name: reply[start:start + count] for name, (start, count) in positions.items()}


def paint(name, row):
    pixel = bytes(COLORS[name])
    data = pixel * (WIDTH * STRIPE_ROWS)
    path = SCRATCH / "stripe.bin"
    path.write_bytes(data)
    address = FRAMEBUFFER + row * WIDTH * BYTES_PER_PIXEL
    start = time.monotonic()
    reply = command(f"load_image {{{path}}} {address:#x} bin")
    if "downloaded" not in reply:
        raise RuntimeError(f"paint failed: {reply[:90]!r}")
    print(f"  {name}: rows {row}-{row + STRIPE_ROWS - 1}, {len(data)} bytes in {time.monotonic() - start:.1f}s", flush=True)
    path.unlink()


halt()
try:
    print("=== the firmware's answers")
    answers = ask_firmware()
    for name, value in answers.items():
        print(f"  {name:22} {' '.join(hex(word) for word in value)}")

    arm_base, arm_size = answers["ARM memory"]
    gpu_base, gpu_size = answers["GPU memory"]
    print(f"\n  CPU's share: {arm_base:#010x}-{arm_base + arm_size:#010x} ({arm_size / 2**20:.0f} MiB)")
    print(f"  GPU's share: {gpu_base:#010x}-{gpu_base + gpu_size:#010x} ({gpu_size / 2**20:.0f} MiB)")
    inside = gpu_base <= FRAMEBUFFER < gpu_base + gpu_size
    print(f"  framebuffer {FRAMEBUFFER:#x} is {'INSIDE' if inside else 'NOT inside'} the GPU's share of RAM")

    print("\n=== what's at the framebuffer now")
    before = read_words(FRAMEBUFFER + STRIPE_START_ROW * WIDTH * BYTES_PER_PIXEL, 4)
    print("  " + " ".join(f"{word:#010x}" for word in before))

    print("\n=== painting; watch the screen", flush=True)
    row = STRIPE_START_ROW
    for name in COLORS:
        paint(name, row)
        row += STRIPE_ROWS + STRIPE_GAP_ROWS
        time.sleep(PAUSE_SECONDS)
    after = read_words(FRAMEBUFFER + STRIPE_START_ROW * WIDTH * BYTES_PER_PIXEL, 4)
    print("  read back: " + " ".join(f"{word:#010x}" for word in after))
finally:
    command("resume")
    print("resumed:", command("bcm2711.cpu0 curstate"))
