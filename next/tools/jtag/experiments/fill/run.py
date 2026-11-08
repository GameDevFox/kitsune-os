#!/usr/bin/env python3
"""Times full-screen fills on the real Pi, using a throwaway copy of the kernel.

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
WORK = SP / "fill-copy"
RESET = HERE.parent / "reset.py"
DONE_TIMEOUT = 60
WIDTH, HEIGHT = 1920, 1080

shutil.rmtree(WORK, ignore_errors=True)
shutil.copytree(SRC, WORK, ignore=shutil.ignore_patterns("target"))
main = WORK / "kernel/src/main.rs"
text = main.read_text()
for old, new in [("mod animation;\n", "mod animation;\nmod experiment;\n"), ("animation::run(&framebuffer)", "experiment::run(&framebuffer)")]:
    assert text.count(old) == 1, old
    text = text.replace(old, new)
main.write_text(text)
shutil.copy(HERE / "experiment.rs", WORK / "kernel/src/experiment.rs")

subprocess.run([sys.executable, str(RESET)], check=True, capture_output=True)

viewer = socket.create_connection(("localhost", 8765), timeout=5)
viewer.settimeout(0.2)
header = b""
while not header.endswith(b"\n"):
    header += viewer.recv(1)
left = int(header.split(b"=")[1])
while left > 0:
    left -= len(viewer.recv(min(left, 65536)))

deploy = subprocess.run(["cargo", "pi"], cwd=WORK, capture_output=True, text=True)
print((deploy.stdout + deploy.stderr).strip()[-500:], flush=True)
if deploy.returncode != 0:
    sys.exit(1)

serial = b""
deadline = time.monotonic() + DONE_TIMEOUT
while b"DONE" not in serial and b"FATAL" not in serial and time.monotonic() < deadline:
    try:
        serial += viewer.recv(65536)
    except TimeoutError:
        pass
time.sleep(0.5)
try:
    serial += viewer.recv(65536)
except TimeoutError:
    pass
text = serial.decode(errors="replace").replace("\r", "")
print(text[text.find("--- bytes"):] if "--- bytes" in text else text[-1500:], flush=True)

time.sleep(1.5)
with urllib.request.urlopen("http://localhost:8766/frame.jpg", timeout=5) as response:
    jpeg = response.read()
raw = subprocess.run(
    ["ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "mjpeg", "-i", "-", "-vf", "scale=1:1,format=rgb24", "-f", "rawvideo", "-"],
    input=jpeg, capture_output=True, check=True,
).stdout
print(f"screen average after the cached fill, R,G,B = {raw[0]},{raw[1]},{raw[2]} (painted 0x18,0x10,0x18)", flush=True)

shutil.rmtree(WORK, ignore_errors=True)
subprocess.run([sys.executable, str(RESET)], check=True, capture_output=True)
restore = subprocess.run(["cargo", "pi"], cwd=SRC, capture_output=True, text=True)
print("restored:", (restore.stdout + restore.stderr).strip().splitlines()[-1])
