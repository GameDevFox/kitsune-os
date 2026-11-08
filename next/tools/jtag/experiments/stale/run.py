#!/usr/bin/env python3
"""Checks that `cargo pi` can't run stale cached code on a core with its caches on.

Usage: run.py SCRATCH_DIR

Builds two kernels that differ by one number in their code, and loads them back and
forth, checking that each boot prints its own number. Then does the same with all the
cleanup removed from boot.s, which should fail, or this test proves nothing. Also
tries removing each half of it, which is only reported, as the halves overlap.
Puts the current kernel back at the end.
"""
import re
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

SP = Path(sys.argv[1])
HERE = Path(__file__).resolve().parent
SRC = HERE.parents[3]
ROOT = SP / "stale-copy"
SERIAL = ("localhost", 8765)
LOADS = 6
WAIT_SECONDS = 6
MARKERS = (0x1234, 0x5678)
BOOT = "kernel/src/arch/aarch64/boot.s"
CLEANING = re.compile(r"    adr     x1, _start\n.*?    isb\n\n(?=    // After `cargo pi`)", re.DOTALL)
ALL_CLEANUP = re.compile(r"    adr     x1, _start\n.*?    isb\n\n(?=    // Only core 0 boots)", re.DOTALL)
MMU_OFF = re.compile(r"    // After `cargo pi`.*?    isb\n\n(?=    // Only core 0 boots)", re.DOTALL)


def prepare(name, marker, removing):
    work = ROOT / name / "next"
    shutil.copytree(SRC, work, ignore=shutil.ignore_patterns("target"))
    shutil.copytree(SRC.parent / "kernel/image", ROOT / name / "kernel/image")
    main = work / "kernel/src/main.rs"
    text = main.read_text()
    old = '    println!("Hello, World!");\n'
    assert text.count(old) == 1
    main.write_text(text.replace(old, old + f'    println!("stale test {{:#x}}", core::hint::black_box({marker:#x}u32));\n'))
    if removing:
        boot = work / BOOT
        text = boot.read_text()
        text, count = removing.subn("", text)
        assert count == 1
        boot.write_text(text)
    # A copy of the encoded images, as the kernel is all that's being tested.
    (work / "target").mkdir()
    shutil.copy2(SRC / "target/assets.bin", work / "target/assets.bin")
    built = subprocess.run(["scripts/build-image"], cwd=work, capture_output=True, text=True)
    if built.returncode:
        sys.exit(built.stdout + built.stderr)
    return work


def reboot():
    result = subprocess.run([sys.executable, str(HERE.parent / "reset.py")], capture_output=True, text=True)
    print("reboot:", (result.stdout + result.stderr).strip()[-80:], flush=True)


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


def load(work, viewer, marker):
    """Returns what this boot printed, and whether it was the right number."""
    deployed = subprocess.run(["cargo", "pi"], cwd=work, capture_output=True, text=True)
    if deployed.returncode:
        return f"cargo pi failed: {(deployed.stdout + deployed.stderr).strip()[-120:]}", False
    serial = b""
    end = time.monotonic() + WAIT_SECONDS
    while time.monotonic() < end:
        try:
            serial += viewer.recv(65536)
        except TimeoutError:
            pass
    text = serial.decode(errors="replace")
    printed = re.findall(r"stale test (0x[0-9a-f]+)", text)
    good = printed == [hex(marker)] and "FATAL" not in text
    return f"expected {marker:#x}, printed {printed}{', FATAL' if 'FATAL' in text else ''}", good


def round_of_loads(viewer, name, removing):
    works = [prepare(f"{name}-{marker:x}", marker, removing) for marker in MARKERS]
    print(f"--- {name}", flush=True)
    right = 0
    # Each load's stale code comes from the one before, so start by running one.
    load(works[1], viewer, MARKERS[1])
    for number in range(LOADS):
        index = number % len(MARKERS)
        outcome, good = load(works[index], viewer, MARKERS[index])
        right += good
        print(f"{'ok   ' if good else 'WRONG'} {outcome}", flush=True)
    print(f"{right} of {LOADS} right", flush=True)
    return right


shutil.rmtree(ROOT, ignore_errors=True)
viewer = open_viewer()
# A kernel left by an earlier run may have no cleaning, and would spoil the first round.
reboot()
kept = round_of_loads(viewer, "nothing removed", None)
no_cleanup = round_of_loads(viewer, "all cleanup removed", ALL_CLEANUP)
# Nothing loaded after a kernel without the cleanup can escape its stale code.
reboot()
no_cleaning = round_of_loads(viewer, "only cleaning by address removed", CLEANING)
reboot()
no_mmu_off = round_of_loads(viewer, "turning the MMU off and cleaning by set and way removed", MMU_OFF)
reboot()
shutil.rmtree(ROOT, ignore_errors=True)
restore = subprocess.run(["cargo", "pi"], cwd=SRC, capture_output=True, text=True)
print("restored:", (restore.stdout + restore.stderr).strip().splitlines()[-1], "exit", restore.returncode, flush=True)
# The two partial rounds are only reported: each leaves the other's `ic iallu` in place.
print(f"partial removals: {no_cleaning} and {no_mmu_off} of {LOADS} right", flush=True)
sys.exit(0 if kept == LOADS and no_cleanup < LOADS and restore.returncode == 0 else 1)
