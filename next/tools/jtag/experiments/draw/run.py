#!/usr/bin/env python3
"""Times drawing each version of Aki at two CPU clocks, using a throwaway copy of the kernel.

Usage: run.py SCRATCH_DIR

Only asks for the firmware's own maximum. Reboots before and after, which also
puts the clock back, and restores the current kernel at the end. The images must be
on the card already. The screen cycles through each version during the run.
"""
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

SP = Path(sys.argv[1])
HERE = Path(__file__).resolve().parent
SRC = HERE.parents[3]
ROOT = SP / "draw-copy"
WORK = ROOT / "next"
RESET = HERE.parent / "reset.py"
DONE_TIMEOUT = 240

shutil.rmtree(ROOT, ignore_errors=True)
shutil.copytree(SRC, WORK, ignore=shutil.ignore_patterns("target"))
# The image sources are outside `next/`, and the build needs them.
shutil.copytree(SRC.parent / "kernel/image", ROOT / "kernel/image")
main = WORK / "kernel/src/main.rs"
text = main.read_text()
for old, new in [("mod animation;\n", "mod animation;\nmod experiment;\n#[allow(dead_code)]\nmod clock;\n#[allow(dead_code)]\nmod stores;\n"), ("animation::run(&animation_framebuffer, |key| pictures::handle_key(&animation_framebuffer, key))", "experiment::run(&animation_framebuffer)")]:
    assert text.count(old) == 1, old
    text = text.replace(old, new)
main.write_text(text)
shutil.copy(HERE / "experiment.rs", WORK / "kernel/src/experiment.rs")
shutil.copy(HERE.parent / "clock/experiment.rs", WORK / "kernel/src/clock.rs")
shutil.copy(HERE.parent / "stores/experiment.rs", WORK / "kernel/src/stores.rs")

result = subprocess.run([sys.executable, str(RESET)], capture_output=True, text=True)
print("reset:", (result.stdout + result.stderr).strip(), flush=True)
if result.returncode != 0:
    sys.exit(1)

viewer = socket.create_connection(("localhost", 8765), timeout=5)
viewer.settimeout(0.2)
header = b""
while not header.endswith(b"\n"):
    header += viewer.recv(1)
left = int(header.split(b"=")[1])
while left > 0:
    left -= len(viewer.recv(min(left, 65536)))

deploy = subprocess.run(["cargo", "pi"], cwd=WORK, capture_output=True, text=True)
print((deploy.stdout + deploy.stderr).strip()[-1500:], flush=True)

failed = deploy.returncode != 0
if not failed:
    serial = b""
    deadline = time.monotonic() + DONE_TIMEOUT
    while b"DONE" not in serial and time.monotonic() < deadline:
        try:
            serial += viewer.recv(65536)
        except TimeoutError:
            pass
    output = serial.decode(errors="replace").replace("\r", "")
    print(output[output.find("=== ARM"):] if "=== ARM" in output else output[-2000:], flush=True)
    failed = b"DONE" not in serial

shutil.rmtree(ROOT, ignore_errors=True)
reset = subprocess.run([sys.executable, str(RESET)], capture_output=True, text=True)
print("reset:", (reset.stdout + reset.stderr).strip(), flush=True)
restore = subprocess.run(["cargo", "pi"], cwd=SRC, capture_output=True, text=True)
print("restored:", (restore.stdout + restore.stderr).strip().splitlines()[-1], "exit", restore.returncode, flush=True)
sys.exit(1 if failed or reset.returncode or restore.returncode else 0)
