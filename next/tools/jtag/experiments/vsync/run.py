#!/usr/bin/env python3
"""Measures flipping and vsync on the real Pi, using a throwaway copy of the kernel.

Usage: run.py SCRATCH_DIR
"""
import shutil
import socket
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

SP = Path(sys.argv[1])
HERE = Path(__file__).resolve().parent
SRC = HERE.parents[3]
WORK = SP / "vsync-copy"
BASE = "http://localhost:8766"
WIDTH, HEIGHT = 1920, 1080
COLUMN = WIDTH // 2
ROW_STEP = 4
NO_SIGNAL_LEVEL = 4
FLIP_TEST_SECONDS = 8
MIN_FRACTION = 0.03
READY_TIMEOUT = 25

EDITS = {
    "kernel/src/board/raspi4/mod.rs": [("mod mailbox;\n", "pub mod mailbox;\n")],
    "kernel/src/board/raspi4/mailbox.rs": [
        # Twice the screen's height, for two buffers.
        ("TAG_SET_VIRTUAL_SIZE, 8, 0, width, height,", "TAG_SET_VIRTUAL_SIZE, 8, 0, width, height * 2,"),
        # Clear any late reply, so that a call can't mistake it for its own.
        ("while read_volatile(MAILBOX_STATUS) & STATUS_FULL != 0 {}\n",
         "while read_volatile(MAILBOX_STATUS) & STATUS_EMPTY == 0 { let _ = read_volatile(MAILBOX_READ); }\n"
         "        while read_volatile(MAILBOX_STATUS) & STATUS_FULL != 0 {}\n"),
        # A timeout, so an unsupported tag reports failure rather than hanging.
        ("while read_volatile(MAILBOX_STATUS) & STATUS_EMPTY != 0 {}\n",
         "let start = super::microseconds();\n"
         "            while read_volatile(MAILBOX_STATUS) & STATUS_EMPTY != 0 {\n"
         "                if super::microseconds().wrapping_sub(start) > 1_000_000 { return false; }\n"
         "            }\n"),
        ("/// Asks the firmware for a framebuffer.",
         "/// Sends one tag with `values`, which the firmware overwrites with its answer.\n"
         "pub fn property(tag: u32, values: &mut [u32]) -> bool {\n"
         "    let mut message = Message([0; MESSAGE_WORDS]);\n"
         "    let size = (values.len() * 4) as u32;\n"
         "    let header = [(MESSAGE_WORDS * 4) as u32, REQUEST, tag, size, 0];\n"
         "    let pointer = message.0.as_mut_ptr();\n"
         "    unsafe {\n"
         "        for (index, word) in header.into_iter().chain(values.iter().copied()).enumerate() {\n"
         "            write_volatile(pointer.add(index), word);\n"
         "        }\n"
         "        write_volatile(pointer.add(header.len() + values.len()), END_TAG);\n"
         "    }\n"
         "    let ok = call(&mut message);\n"
         "    for (index, value) in values.iter_mut().enumerate() {\n"
         "        *value = unsafe { read_volatile(message.0.as_ptr().add(header.len() + index)) };\n"
         "    }\n"
         "    ok\n"
         "}\n\n"
         "/// Asks the firmware for a framebuffer."),
    ],
    "kernel/src/main.rs": [
        ("mod animation;\n", "mod animation;\nmod experiment;\n"),
        ("animation::run(&framebuffer)", "experiment::run(&framebuffer)"),
    ],
}


def send(payload):
    request = urllib.request.Request(BASE + "/send", data=payload, method="POST", headers={"Content-Type": "application/octet-stream"})
    with urllib.request.urlopen(request, timeout=5) as response:
        assert response.status == 200


def classify():
    """Returns how much of a frame's middle column is reddish and how much bluish."""
    with urllib.request.urlopen(BASE + "/frame.jpg", timeout=5) as response:
        jpeg = response.read()
    raw = subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "mjpeg", "-i", "-",
         "-vf", f"crop=2:{HEIGHT}:{COLUMN}:0", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        input=jpeg, capture_output=True, check=True,
    ).stdout
    rows = [(raw[y * 6], raw[y * 6 + 2]) for y in range(0, HEIGHT, ROW_STEP)]
    red = sum(r > b + 8 for r, b in rows)
    blue = sum(b > r + 8 for r, b in rows)
    return red / len(rows), blue / len(rows), rows, jpeg


def letter():
    """B or R for a screen of that color, M for both, K for no signal, ? otherwise."""
    red, blue, rows, jpeg = classify()
    average = sum(r + b for r, b in rows) / len(rows) / 2
    if average < NO_SIGNAL_LEVEL:
        return "K", jpeg
    if red > MIN_FRACTION and blue > MIN_FRACTION:
        return "M", jpeg
    return ("R" if red > blue else "B") if max(red, blue) > MIN_FRACTION else "?", jpeg


def timeline(seconds):
    letters = ""
    saved = set()
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        code, jpeg = letter()
        letters += code
        if code == "M" and "M" not in saved:
            (SP / "tear.jpg").write_bytes(jpeg)
            saved.add("M")
    return letters


def serial_text(viewer, seconds=0.3):
    data = b""
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        try:
            data += viewer.recv(65536)
        except TimeoutError:
            pass
    return data.decode(errors="replace").replace("\r", "").strip()


shutil.rmtree(WORK, ignore_errors=True)
shutil.copytree(SRC, WORK, ignore=shutil.ignore_patterns("target"))
for path, edits in EDITS.items():
    file = WORK / path
    text = file.read_text()
    for old, new in edits:
        assert text.count(old) == 1, f"{path}: {old!r}: {text.count(old)}"
        text = text.replace(old, new)
    file.write_text(text)
shutil.copy(HERE / "experiment.rs", WORK / "kernel/src/experiment.rs")

viewer = socket.create_connection(("localhost", 8765), timeout=5)
viewer.settimeout(0.1)
header = b""
while not header.endswith(b"\n"):
    header += viewer.recv(1)
left = int(header.split(b"=")[1])
while left > 0:
    left -= len(viewer.recv(min(left, 65536)))



def fresh_start():
    """A clean slate for each test: reboot, then load the experiment."""
    reset = subprocess.run([sys.executable, str(HERE.parent / "reset.py")], capture_output=True, text=True)
    if reset.returncode != 0:
        raise SystemExit(f"reset failed: {reset.stdout}{reset.stderr}")
    deploy = subprocess.run(["cargo", "pi"], cwd=WORK, capture_output=True, text=True)
    if deploy.returncode != 0:
        raise SystemExit(f"deploy failed: {deploy.stdout}{deploy.stderr}")
    serial = serial_text(viewer, 4)
    if "READY" not in serial:
        raise SystemExit(f"never reached READY: {serial[-300:]}")
    return serial


TESTS = [
    ("slow flips, synced to the line counter", b"L", 20),
    ("slow flips, not synced", b"F", 20),
    ("flips every frame, synced, long run", b"l", 40),
]
for name, command, seconds in TESTS:
    print(f"=== {name}, from a clean start", flush=True)
    fresh_start()
    print("before:", timeline(1), flush=True)
    send(command)
    letters = timeline(seconds)
    print(letters[:150], flush=True)
    print({code: letters.count(code) for code in "BRMK?"}, flush=True)
    send(b"h")
    print("after stopping:", timeline(2)[-30:], flush=True)

shutil.rmtree(WORK, ignore_errors=True)
subprocess.run([sys.executable, str(HERE.parent / "reset.py")], check=True, capture_output=True)
restore = subprocess.run(["cargo", "pi"], cwd=SRC, capture_output=True, text=True)
print("restored:", (restore.stdout + restore.stderr).strip().splitlines()[-1])
