#!/usr/bin/env python3
"""Tests writing to the card through the filesystem, on `update-test.bin` only.

Usage: run.py SCRATCH_DIR

1. Boots a kernel that overwrites the file, reads it back and compares it.
2. Reboots through the watchdog, which resets the card's controller and leaves the card
   powered, then boots a kernel that only compares the card with what was written.
3. Puts the current kernel back.
"""
import os
import shutil
import socket
import subprocess
import sys
import time
from pathlib import Path

SP = Path(sys.argv[1])
HERE = Path(__file__).resolve().parent
SRC = HERE.parents[3]
ROOT = SP / "sd-overwrite-copy"
WORK = ROOT / "next"
RESET = HERE.parent / "reset.py"
ANIMATION_CALL = "animation::run(&animation_framebuffer, |key| pictures::handle_key(&animation_framebuffer, &mut assets, key))"
WAIT_SECONDS = 60

shutil.rmtree(ROOT, ignore_errors=True)
shutil.copytree(SRC, WORK, ignore=shutil.ignore_patterns("target", "__pycache__"))
shutil.copytree(SRC.parent / "kernel/image", ROOT / "kernel/image")
main = WORK / "kernel/src/main.rs"
text = main.read_text()
for old, new in [("mod animation;\n", "mod animation;\nmod experiment;\n"), (ANIMATION_CALL, "experiment::run(&animation_framebuffer)")]:
    assert text.count(old) == 1, old
    text = text.replace(old, new)
main.write_text(text.replace("let mut assets", "#[allow(unused_mut, unused_variables)] let mut assets", 1))
shutil.copy(HERE / "experiment.rs", WORK / "kernel/src/experiment.rs")
(WORK / "target").mkdir()
shutil.copy2(SRC / "target/assets.bin", WORK / "target/assets.bin")


def viewer():
    connection = socket.create_connection(("localhost", 8765), timeout=5)
    connection.settimeout(0.2)
    header = b""
    while not header.endswith(b"\n"):
        header += connection.recv(1)
    left = int(header.split(b"=")[1])
    while left > 0:
        left -= len(connection.recv(min(left, 65536)))
    return connection


def run_stage(environment):
    connection = viewer()
    deployed = subprocess.run(["cargo", "pi"], cwd=WORK, env={**os.environ, **environment}, capture_output=True, text=True)
    print((deployed.stdout + deployed.stderr).strip().splitlines()[-1], flush=True)
    if deployed.returncode:
        return ""
    serial = b""
    deadline = time.monotonic() + WAIT_SECONDS
    while b"DONE" not in serial and time.monotonic() < deadline:
        try:
            serial += connection.recv(65536)
        except TimeoutError:
            pass
    output = serial.decode(errors="replace").replace("\r", "")
    print(output[output.find("=== overwrite"):], flush=True)
    return output


def reboot():
    result = subprocess.run([sys.executable, str(RESET)], capture_output=True, text=True)
    print("reboot:", (result.stdout + result.stderr).strip()[-70:], flush=True)
    return result.returncode == 0


good = reboot()
output = run_stage({})
good = good and "compared" in output and "Ok(())" in output and "refused: false" not in output
if good and reboot():
    output = run_stage({"OVERWRITE_CHECK_ONLY": "CHECK"})
    good = "holds what was written: Ok(())" in output
else:
    good = False

shutil.rmtree(ROOT, ignore_errors=True)
restore = subprocess.run(["cargo", "pi"], cwd=SRC, capture_output=True, text=True)
print("restored:", (restore.stdout + restore.stderr).strip().splitlines()[-1], "exit", restore.returncode, flush=True)
sys.exit(0 if good and restore.returncode == 0 else 1)
