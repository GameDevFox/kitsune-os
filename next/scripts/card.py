#!/usr/bin/env python3
"""Chooses which kernel the shared SD card boots, keeping both on the card.

  scripts/card.py status          Show which kernel the card will boot
  scripts/card.py new             Install target/kitsune-next.img and boot it
  scripts/card.py new-kernel-only Install just the kernel under that name, padded to a
                                  fixed size so that it can be updated in place, with
                                  a trial copy, its tryboot.txt, and a scratch file for
                                  testing, and boot it. It needs the images in raw blocks
  scripts/card.py old             Boot the old 32-bit kernel again

The card is found by its filesystem UUID, and is only written if it is removable.
"""

import hashlib
import os
import shutil
import subprocess
import sys
from contextlib import contextmanager
from pathlib import Path

CARD_UUID = "AE82-4BC1"
CARD_LABEL = "boot"

CONFIG = "config.txt"
NEW_KERNEL = "kitsune-next.img"
OLD_KERNEL = "kitsune.img"
KERNEL_KEY = "kernel="
ARM_64BIT_LINE = "arm_64bit=1"
LOCAL_IMAGE = Path(__file__).resolve().parent.parent / "target" / NEW_KERNEL
LOCAL_KERNEL = LOCAL_IMAGE.with_name("kernel-only.img")
# The kernel can only overwrite a file in place, so this is the most it can ever update
# itself to without another trip through this script. The firmware reads all of it at boot,
# at about 5.5 MB/s.
KERNEL_FILE_BYTES = 512 * 1024
# Somewhere for the kernel to test writing, that nothing boots from.
SCRATCH_FILE = "update-test.bin"
# What the firmware boots once, after the kernel asks it to, so a bad kernel can be tried safely.
TRIAL_KERNEL = "kitsune-try.img"
TRIAL_CONFIG = "tryboot.txt"


class CardError(Exception):
    pass


def padded(contents, size):
    """Returns the contents followed by zeros, to exactly `size` bytes."""
    if len(contents) > size:
        raise CardError(f"{len(contents)} bytes doesn't fit in {size}")
    return contents + bytes(size - len(contents))


def switch_config(text, kernel, arm_64bit):
    """Returns config.txt with exactly one active kernel line, set as requested."""
    if "\r" in text:
        raise CardError("config.txt has CRLF line endings")
    lines = text.split("\n")
    kernel_lines = [index for index, line in enumerate(lines) if line.startswith(KERNEL_KEY)]
    if len(kernel_lines) != 1:
        raise CardError(f"expected one active kernel line, found {len(kernel_lines)}")

    lines[kernel_lines[0]] = KERNEL_KEY + kernel
    if arm_64bit:
        if ARM_64BIT_LINE not in lines:
            lines.insert(kernel_lines[0] + 1, ARM_64BIT_LINE)
    else:
        lines = [line for line in lines if line != ARM_64BIT_LINE]
    return "\n".join(lines)


def active_kernel(text):
    lines = text.split("\n")
    kernels = [line[len(KERNEL_KEY):] for line in lines if line.startswith(KERNEL_KEY)]
    return kernels, ARM_64BIT_LINE in lines


def run(*command):
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        raise CardError(f"{' '.join(command)}: {result.stderr.strip() or result.stdout.strip()}")
    return result.stdout.strip()


def find_partition():
    for line in run("lsblk", "-rno", "NAME,UUID").splitlines():
        parts = line.split()
        if len(parts) == 2 and parts[1] == CARD_UUID:
            partition = f"/dev/{parts[0]}"
            break
    else:
        raise CardError(f"no card with UUID {CARD_UUID}. Is it in the reader?")

    disk = run("lsblk", "-no", "PKNAME", partition).splitlines()[0]
    if run("lsblk", "-dno", "RM", f"/dev/{disk}").strip() != "1":
        raise CardError(f"/dev/{disk} is not removable")
    label = run("lsblk", "-no", "LABEL", partition)
    if label != CARD_LABEL:
        raise CardError(f"unexpected label: {label}")
    return partition


def mounted_options(partition):
    for line in Path("/proc/mounts").read_text().splitlines():
        fields = line.split()
        if fields[0] == partition:
            return fields[3].split(",")
    return None


@contextmanager
def mounted(partition, writable):
    if mounted_options(partition) is not None:
        raise CardError(f"{partition} is already mounted; please unmount it first")

    options = [] if writable else ["-o", "ro"]
    run("udisksctl", "mount", "--no-user-interaction", "-b", partition, *options)
    try:
        if not writable and "ro" not in mounted_options(partition):
            raise CardError("the read-only mount is writable")
        yield Path(run("lsblk", "-no", "MOUNTPOINT", partition).splitlines()[0])
    finally:
        os.sync()
        run("udisksctl", "unmount", "--no-user-interaction", "-b", partition)


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def status(partition):
    with mounted(partition, writable=False) as card:
        kernels, arm_64bit = active_kernel((card / CONFIG).read_text())
        for kernel in kernels:
            exists = "present" if (card / kernel).exists() else "MISSING"
            print(f"boots: {kernel} ({exists}), arm_64bit={'on' if arm_64bit else 'off'}")
        return (card / CONFIG).read_text()


def write_config(card, kernel, arm_64bit):
    path = card / CONFIG
    text = switch_config(path.read_text(newline=""), kernel, arm_64bit)
    path.write_text(text, newline="")
    return text


def boot_new(partition, local_image=LOCAL_IMAGE):
    if not local_image.exists():
        raise CardError(f"{local_image} is missing; run scripts/build-image")
    expected_image = sha256(local_image)

    with mounted(partition, writable=True) as card:
        if not (card / OLD_KERNEL).exists():
            raise CardError(f"{OLD_KERNEL} is missing from the card; stopping")
        shutil.copyfile(local_image, card / NEW_KERNEL)
        expected_config = write_config(card, NEW_KERNEL, arm_64bit=True)

    with mounted(partition, writable=False) as card:
        if sha256(card / NEW_KERNEL) != expected_image:
            raise CardError("the image on the card does not match")
        if (card / CONFIG).read_text(newline="") != expected_config:
            raise CardError("config.txt on the card does not match")
    print(f"installed {NEW_KERNEL} ({local_image.stat().st_size} bytes), verified from a fresh mount")


def boot_old(partition):
    with mounted(partition, writable=True) as card:
        if not (card / OLD_KERNEL).exists():
            raise CardError(f"{OLD_KERNEL} is missing from the card; stopping")
        expected_config = write_config(card, OLD_KERNEL, arm_64bit=False)

    with mounted(partition, writable=False) as card:
        if (card / CONFIG).read_text(newline="") != expected_config:
            raise CardError("config.txt on the card does not match")


def boot_new_kernel_only(partition):
    if not LOCAL_KERNEL.exists():
        raise CardError(f"{LOCAL_KERNEL} is missing; run scripts/build-image")
    kernel = padded(LOCAL_KERNEL.read_bytes(), KERNEL_FILE_BYTES)
    scratch = bytes(KERNEL_FILE_BYTES)
    wanted = {NEW_KERNEL: kernel, TRIAL_KERNEL: kernel, SCRATCH_FILE: scratch}

    with mounted(partition, writable=True) as card:
        if not (card / OLD_KERNEL).exists():
            raise CardError(f"{OLD_KERNEL} is missing from the card; stopping")
        for name, contents in wanted.items():
            (card / name).write_bytes(contents)
        expected_config = write_config(card, NEW_KERNEL, arm_64bit=True)
        # Everything else about a trial boot is the same as a normal one.
        trial_config = switch_config(expected_config, TRIAL_KERNEL, True)
        (card / TRIAL_CONFIG).write_text(trial_config, newline="")

    with mounted(partition, writable=False) as card:
        for name, contents in wanted.items():
            if (card / name).read_bytes() != contents:
                raise CardError(f"{name} on the card does not match")
        if (card / TRIAL_CONFIG).read_text(newline="") != trial_config:
            raise CardError(f"{TRIAL_CONFIG} on the card does not match")
        if (card / CONFIG).read_text(newline="") != expected_config:
            raise CardError("config.txt on the card does not match")
    print(f"installed {', '.join(wanted)}, {KERNEL_FILE_BYTES} bytes each, and {TRIAL_CONFIG}, verified from a fresh mount")


COMMANDS = {"status": status, "new": boot_new, "new-kernel-only": boot_new_kernel_only, "old": boot_old}


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in COMMANDS:
        print(__doc__)
        return 1
    try:
        partition = find_partition()
        COMMANDS[sys.argv[1]](partition)
        if sys.argv[1] != "status":
            status(partition)
    except CardError as error:
        print(f"card: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
