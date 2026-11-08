#!/usr/bin/env python3
"""Cuts a clip from a recording named in `voice.local.json`, and loads it into the Pi's
sound region over JTAG, where key 8 plays it.

Usage: voice.py [LINE]     LINE is a name from the cue file, if it has one
       voice.py --list     Show the line names

No recording is in this repo, nor named by it: each machine says which to use in
`voice.local.json`, beside this tool's folder, which git ignores. For example:

    {"recording": "~/sounds/hello.opus", "offset": 1.5, "duration": 2.0}

`offset` and `duration` are in seconds, and default to the whole recording. Or, for a
recording of many lines, a cue file can name each by its offset and duration, in entries
like `"ok-lets-do-this": { offset: 9.668, duration: 2.0 },` (lines starting `//` are
skipped), and `line` picks the one for the card:

    {"recording": "~/sounds/momo.opus", "cues": "~/sounds/momo.ts", "line": "ok-lets-do-this"}

The cut clip goes to `target/voice.bin`. A clip is little-endian: the magic bytes, its
sample rate, its number of samples, then that many signed 16-bit mono samples, which
`kernel/src/sound.rs` must agree on. RAM keeps it across a watchdog reboot and `cargo
pi`, but not a power cycle, and the kernel plays it in place of the card's clip.

`scripts/make-assets.py` also puts the settings' own line in with the images, through
`card_clip`, so it's on the card, and survives a power cycle.
"""
import array
import json
import re
import socket
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

NEXT = Path(__file__).resolve().parent.parent
SETTINGS = NEXT / "voice.local.json"
OUTPUT = NEXT / "target/voice.bin"

MAGIC = b"KITSND01"
HEADER = "<II"
HEADER_BYTES = len(MAGIC) + struct.calcsize(HEADER)
# The PWM's own rate, `SAMPLE_RATE` in `board/raspi4/audio.rs`: 27 MHz / 562 is 48,042.7,
# which the kernel rounds down. The kernel refuses a clip at any other rate.
SAMPLE_RATE = 48042
SAMPLE_BYTES = 2
# Recordings are often quiet, and the PWM has only about 9 bits, so each clip is scaled
# to peak at this much of full scale.
PEAK_FRACTION = 0.9
FULL_SCALE = 32767
# `SOUND_BASE` and `SOUND_SIZE` in `board/raspi4/mod.rs`.
SOUND_ADDRESS = 0x1200_0000
SOUND_BYTES = 4 * 1024 * 1024

TCL = ("127.0.0.1", 6666)
END = b"\x1a"


def settings():
    if not SETTINGS.exists():
        sys.exit(f"no {SETTINGS.name}: make one naming a recording, as `voice.py` describes")
    found = json.loads(SETTINGS.read_text())
    if "recording" not in found:
        sys.exit(f"{SETTINGS.name} names no recording")
    return found


def lines(cues):
    """Each line's name and its chosen take's offset and duration, in seconds. Other
    takes are commented out in a cue file, so only uncommented values count."""
    text = cues.read_text()
    found = {}
    for match in re.finditer(r'^\s*"([a-z0-9-]+)": \{(.*?)^\s*\},', text, re.MULTILINE | re.DOTALL):
        name, body = match.groups()
        live = "\n".join(line for line in body.splitlines() if not line.strip().startswith("//"))
        offset = re.search(r"offset:\s*([\d.]+)", live)
        duration = re.search(r"duration:\s*([\d.]+)", live)
        if offset and duration:
            found[name] = (float(offset.group(1)), float(duration.group(1)))
    return found


def louder(samples):
    """Scales the samples so the loudest peaks at `PEAK_FRACTION` of full scale."""
    values = array.array("h", samples)
    peak = max(abs(value) for value in values) or 1
    target = int(FULL_SCALE * PEAK_FRACTION)
    return array.array("h", (value * target // peak for value in values)).tobytes()


def cut(recording, offset, duration):
    """The clip as signed 16-bit mono samples at the PWM's rate, all of the recording
    from `offset` if `duration` is None."""
    length = [] if duration is None else ["-t", str(duration)]
    result = subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-ss", str(offset), *length,
         "-i", str(recording), "-ac", "1", "-ar", str(SAMPLE_RATE), "-f", "s16le", "-"],
        capture_output=True, check=True,
    )
    return result.stdout


def command(sock, text):
    sock.sendall(text.encode() + END)
    data = b""
    while END not in data:
        chunk = sock.recv(65536)
        if not chunk:
            sys.exit("openocd closed the connection")
        data += chunk
    return data.split(END)[0].decode(errors="replace").strip()


def load(clip):
    """Writes the clip to the sound region with core 0 halted, reads it back to check
    it, then lets the kernel carry on."""
    sock = socket.create_connection(TCL, timeout=60)
    command(sock, "targets bcm2711.cpu0")
    command(sock, "halt 2000")
    if command(sock, "bcm2711.cpu0 curstate") != "halted":
        sys.exit("core 0 did not halt")
    try:
        with tempfile.TemporaryDirectory() as scratch:
            sent, back = Path(scratch) / "clip.bin", Path(scratch) / "back.bin"
            sent.write_bytes(clip)
            command(sock, f"load_image {{{sent}}} {SOUND_ADDRESS:#x} bin")
            command(sock, f"dump_image {{{back}}} {SOUND_ADDRESS:#x} {len(clip)}")
            if not back.exists() or back.read_bytes() != clip:
                sys.exit("the clip read back differently, so the kernel may play noise")
    finally:
        command(sock, "resume")


def sources(chosen):
    """The files the settings read, so a build can tell when the clip needs cutting again."""
    return [SETTINGS] + [Path(chosen[key]).expanduser() for key in ("recording", "cues") if key in chosen]


def choose(chosen, requested=None):
    """The clip's name, offset and duration, from the settings and any line requested."""
    recording = Path(chosen["recording"]).expanduser()
    if "cues" not in chosen:
        if requested:
            sys.exit(f"{SETTINGS.name} has no cue file, so there are no lines to choose")
        return recording, recording.name, chosen.get("offset", 0), chosen.get("duration")
    found = lines(Path(chosen["cues"]).expanduser())
    if requested == "--list":
        for name, (offset, duration) in found.items():
            print(f"{name:60} {duration:5.2f} s at {offset:.3f} s")
        sys.exit(0)
    name = requested or chosen.get("line")
    if name not in found:
        sys.exit(f"no line called {name}; --list shows them")
    return recording, name, *found[name]


def make_clip(chosen, requested=None):
    """The clip's name and its bytes, header and all."""
    recording, name, offset, duration = choose(chosen, requested)
    samples = louder(cut(recording, offset, duration))
    return name, MAGIC + struct.pack(HEADER, SAMPLE_RATE, len(samples) // SAMPLE_BYTES) + samples


def card_clip():
    """The settings' own line, for the card, with the files it came from. With no
    settings, there's no clip, and nothing to watch but where the settings would go."""
    if not SETTINGS.exists():
        return None, b"", [SETTINGS]
    chosen = settings()
    name, clip = make_clip(chosen)
    return name, clip, sources(chosen)


def main():
    name, clip = make_clip(settings(), sys.argv[1] if len(sys.argv) > 1 else None)
    duration = (len(clip) - HEADER_BYTES) / SAMPLE_BYTES / SAMPLE_RATE
    if len(clip) > SOUND_BYTES:
        sys.exit(f"{name} is {len(clip)} bytes, more than the sound region's {SOUND_BYTES}")
    OUTPUT.write_bytes(clip)
    load(clip)
    print(f"loaded {name}: {duration:.2f} s, {len(clip) // 1024} KB at {SOUND_ADDRESS:#x}. Press 8 to play it.")


if __name__ == "__main__":
    main()
