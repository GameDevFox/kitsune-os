#!/usr/bin/env python3
"""Copies the images to raw blocks on the card, then tests loading them from there.

Usage: run.py SCRATCH_DIR

1. Reboots, so RAM holds the images from the image file, and copies them to the card.
2. Boots a kernel that first wipes the images from RAM, so anything drawn must have come
   from the card, then presses each key and compares the screen's shapes.
3. Puts the current kernel back.
"""
import importlib.util
import re
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
ROOT = SP / "sd-images-copy"
RESET = HERE.parent / "reset.py"
ANIMATION_CALL = "animation::run(&animation_framebuffer, |key| pictures::handle_key(&animation_framebuffer, &mut assets, key))"
FIND_IMAGES = "    let mut assets = assets::Assets::find("
WAIT_SECONDS = 60
AFTER_KEY_SECONDS = 2

spec = importlib.util.spec_from_file_location("check_images", SRC / "tools/check_images.py")
check_images = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check_images)


def prepare(name, change):
    work = ROOT / name / "next"
    shutil.copytree(SRC, work, ignore=shutil.ignore_patterns("target"))
    shutil.copytree(SRC.parent / "kernel/image", ROOT / name / "kernel/image")
    main = work / "kernel/src/main.rs"
    main.write_text(change(main.read_text(), work))
    (work / "target").mkdir()
    shutil.copy2(SRC / "target/assets.bin", work / "target/assets.bin")
    built = subprocess.run(["scripts/build-image"], cwd=work, capture_output=True, text=True)
    if built.returncode:
        sys.exit((built.stdout + built.stderr)[-1500:])
    return work


def copying(text, work):
    shutil.copy(HERE / "experiment.rs", work / "kernel/src/experiment.rs")
    for old, new in [("mod animation;\n", "mod animation;\nmod experiment;\n"), (ANIMATION_CALL, "experiment::run(&animation_framebuffer)")]:
        assert text.count(old) == 1, old
        text = text.replace(old, new)
    return text.replace("let mut assets", "#[allow(unused_mut, unused_variables)] let mut assets", 1)


def wiping(text, work):
    assert text.count(FIND_IMAGES) == 1
    return text.replace(FIND_IMAGES, "    board::assets_region().fill(0);\n" + FIND_IMAGES)


def open_viewer():
    viewer = socket.create_connection(("localhost", 8765), timeout=5)
    viewer.settimeout(0.2)
    header = b""
    while not header.endswith(b"\n"):
        header += viewer.recv(1)
    left = int(header.split(b"=")[1])
    while left > 0:
        left -= len(viewer.recv(min(left, 65536)))
    return viewer


def deploy(work, viewer, until):
    done = subprocess.run(["cargo", "pi"], cwd=work, capture_output=True, text=True)
    print((done.stdout + done.stderr).strip().splitlines()[-1], flush=True)
    if done.returncode:
        return None
    serial = b""
    deadline = time.monotonic() + WAIT_SECONDS
    while until not in serial and time.monotonic() < deadline:
        try:
            serial += viewer.recv(65536)
        except TimeoutError:
            pass
    return serial.decode(errors="replace").replace("\r", "")


def press(key):
    request = urllib.request.Request("http://localhost:8766/key", data=key, method="POST")
    urllib.request.urlopen(request, timeout=5).read()
    time.sleep(AFTER_KEY_SECONDS)


def shape_errors(pictures):
    return check_images.compare_shapes(check_images.expected(pictures), check_images.screen_from_pi())


def serial_since(viewer):
    serial = b""
    try:
        while True:
            serial += viewer.recv(65536)
    except TimeoutError:
        pass
    return serial.decode(errors="replace").replace("\r", "")


shutil.rmtree(ROOT, ignore_errors=True)
copier = prepare("copy", copying)
wiper = prepare("wipe", wiping)
result = subprocess.run([sys.executable, str(RESET)], capture_output=True, text=True)
print("reset:", (result.stdout + result.stderr).strip(), flush=True)
if result.returncode:
    sys.exit(1)

viewer = open_viewer()
output = deploy(copier, viewer, b"DONE") or ""
print(output[output.find("=== copying"):], flush=True)
good = "read back: matches" in output

if good:
    output = deploy(wiper, viewer, b"Keys:") or ""
    print("\n".join(line for line in output.splitlines() if re.match(r"Images (are|drawn)|Read |Couldn't|FATAL|No images|Kernel started", line)), flush=True)
    time.sleep(AFTER_KEY_SECONDS)
    for name, fraction in shape_errors(check_images.STARTUP):
        print(f"boot, {name}: {fraction:.2%} of pixels don't match", flush=True)
        good = good and fraction <= check_images.CAPTURE_MAX_WRONG
    good = good and "Images are on the SD card" in output

    for key in check_images.AKI_VERSIONS:
        press(check_images.CLEAR_KEY)
        serial_since(viewer)
        press(key)
        loaded = [line for line in serial_since(viewer).splitlines() if re.search(r"Read |Couldn't|FATAL", line)]
        for name, fraction in shape_errors(check_images.aki_pictures(key)):
            print(f"key {key.decode()}, {name}: {fraction:.2%} of pixels don't match", flush=True)
            good = good and fraction <= check_images.CAPTURE_MAX_WRONG
        print(f"  {loaded}", flush=True)
    press(check_images.CLEAR_KEY)
    serial_since(viewer)
    press(b"1")
    print(f"key 1 again, which shouldn't read: {[line for line in serial_since(viewer).splitlines() if re.search(r'Read ', line)]}", flush=True)

shutil.rmtree(ROOT, ignore_errors=True)
restore = subprocess.run(["cargo", "pi"], cwd=SRC, capture_output=True, text=True)
print("restored:", (restore.stdout + restore.stderr).strip().splitlines()[-1], "exit", restore.returncode, flush=True)
sys.exit(0 if good and restore.returncode == 0 else 1)
