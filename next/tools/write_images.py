#!/usr/bin/env python3
"""Writes the images in target/assets.bin to the card's raw image area, over JTAG.

Usage: write_images.py SCRATCH_DIR

Run `scripts/build-image` first. With core 0 halted, this loads the images into their
region of RAM and reads them back, then runs a throwaway kernel that copies that region
to the card, using `tools/jtag/experiments/sd_images/experiment.rs`. It only writes
inside the area, and only if it's blank or already holds images. Afterwards it reads the
region back again, so a stale cache line written over it would show, then reloads the
kernel from this tree with `cargo pi`. Nothing is rebooted, and the card stays put.

The copier must not look up images first: that reads the card's old ones into RAM over
the new ones, as the card comes first.
"""
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

NEXT = Path(__file__).resolve().parent.parent
ASSETS = NEXT / "target/assets.bin"
COPIER = NEXT / "tools/jtag/experiments/sd_images/experiment.rs"
# Must match ASSETS_BASE in `board/raspi4/mod.rs`.
ASSETS_ADDRESS = 0x200000
TCL = ("127.0.0.1", 6666)
VIEWER = ("127.0.0.1", 8765)
END = b"\x1a"
JTAG_TIMEOUT_SECONDS = 300
COPY_TIMEOUT_SECONDS = 60
PATCHES = [
    ("mod animation;\n", "mod animation;\nmod experiment;\n"),
    ("animation::run(&animation_framebuffer, logo_drawn, &mut assets)",
     "experiment::run(&animation_framebuffer)"),
    ("    let mut assets = assets::Assets::find(board::assets_region(), card::image_area());",
     "    #[allow(unused_mut, unused_variables)] let mut assets: Option<assets::Assets> = None;"),
]


def jtag():
    sock = socket.create_connection(TCL, timeout=JTAG_TIMEOUT_SECONDS)

    def command(text):
        sock.sendall(text.encode() + END)
        data = b""
        while END not in data:
            chunk = sock.recv(65536)
            if not chunk:
                sys.exit("openocd closed the connection")
            data += chunk
        return data.split(END)[0].decode(errors="replace").strip()

    command("targets bcm2711.cpu0")
    command("halt 2000")
    if command("bcm2711.cpu0 curstate") != "halted":
        sys.exit("core 0 did not halt")
    return command


def region_matches(command, scratch):
    """Reads the image region back with core 0 halted, and compares it with the build."""
    back = scratch / "region.bin"
    back.unlink(missing_ok=True)
    command(f"dump_image {{{back}}} {ASSETS_ADDRESS:#x} {ASSETS.stat().st_size}")
    return back.exists() and back.read_bytes() == ASSETS.read_bytes()


def build_copier(scratch):
    root = scratch / "write-images"
    shutil.rmtree(root, ignore_errors=True)
    work = root / "next"
    shutil.copytree(NEXT, work, ignore=shutil.ignore_patterns("target"))
    (root / "kernel").symlink_to(NEXT.parent / "kernel")
    shutil.copy(COPIER, work / "kernel/src/experiment.rs")
    main = work / "kernel/src/main.rs"
    text = main.read_text()
    for old, new in PATCHES:
        if text.count(old) != 1:
            sys.exit(f"main.rs has changed, so the copier can't be made: {old!r}")
        text = text.replace(old, new)
    main.write_text(text)
    return work


def run_copier(work):
    viewer = socket.create_connection(VIEWER, timeout=5)
    viewer.settimeout(0.2)
    header = b""
    while not header.endswith(b"\n"):
        header += viewer.recv(1)
    history = int(header.split(b"=")[1])
    while history > 0:
        history -= len(viewer.recv(min(history, 65536)))
    done = subprocess.run(["cargo", "pi"], cwd=work, capture_output=True, text=True)
    if done.returncode:
        sys.exit("the copier didn't load:\n" + (done.stdout + done.stderr)[-1500:])
    serial = b""
    deadline = time.monotonic() + COPY_TIMEOUT_SECONDS
    while b"DONE" not in serial and time.monotonic() < deadline:
        try:
            serial += viewer.recv(65536)
        except TimeoutError:
            pass
    report = serial.decode(errors="replace").replace("\r", "")
    return report[report.find("=== copying"):]


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    scratch = Path(sys.argv[1])
    if not ASSETS.exists():
        sys.exit("no target/assets.bin: run scripts/build-image first")
    work = build_copier(scratch)
    command = jtag()
    print(command(f"load_image {{{ASSETS}}} {ASSETS_ADDRESS:#x} bin"), flush=True)
    if not region_matches(command, scratch):
        sys.exit("the images read back differently from RAM, so nothing was written")
    report = run_copier(work)
    print(report, flush=True)
    written = "read back: matches" in report
    command = jtag()
    intact = region_matches(command, scratch)
    print(f"the region still holds this build's images: {intact}", flush=True)
    shutil.rmtree(work.parent, ignore_errors=True)
    restore = subprocess.run(["cargo", "pi"], cwd=NEXT, capture_output=True, text=True)
    print((restore.stdout + restore.stderr).strip().splitlines()[-1], flush=True)
    good = written and intact and restore.returncode == 0
    print("WRITTEN" if good else "FAILED")
    sys.exit(0 if good else 1)


main()
