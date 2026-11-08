"""Records this PC's line-in, where the Pi's headphone jack is wired, and checks which
pitch each side carries. Used by `check_tone.py` and `jtag/experiments/tone.py`."""
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

import numpy as np

DEVICE = "alsa_input.usb-Generic_USB_Audio-00.HiFi__Line__source"
SAMPLE_RATE = 48000
SIDES = ("left", "right")
# parecord runs until stopped, so it gets a little longer to start up.
STARTUP_SECONDS = 1
# A recorded peak this close to the expected pitch counts.
PITCH_TOLERANCE_HZ = 2
# The other side's pitch must be at least this much quieter, so they aren't bridged.
MIN_SEPARATION_DB = 20
DECIBELS = 20


def start_recording(path):
    return subprocess.Popen(
        ["parecord", f"--device={DEVICE}", "--file-format=wav", "--format=s16le",
         f"--rate={SAMPLE_RATE}", "--channels=2", str(path)],
    )


def finish_recording(recorder, path, seconds):
    """Lets the recording run for `seconds` from its start, then returns its samples,
    one row per frame. parecord only finishes its file's header when asked to stop,
    rather than killed, so this stops it politely."""
    try:
        recorder.wait(timeout=seconds + STARTUP_SECONDS)
        sys.exit(f"parecord stopped early, with status {recorder.returncode}")
    except subprocess.TimeoutExpired:
        recorder.terminate()
        recorder.wait()
    with wave.open(str(path)) as recording:
        raw = recording.readframes(recording.getnframes())
    return np.frombuffer(raw, dtype=np.int16).reshape(-1, len(SIDES)).astype(np.float64)


def record_during(seconds, action):
    """Records for `seconds`, running `action` once the recording has started."""
    with tempfile.TemporaryDirectory() as scratch:
        path = Path(scratch) / "line-in.wav"
        recorder = start_recording(path)
        try:
            action()
        except BaseException:
            recorder.terminate()
            raise
        return finish_recording(recorder, path, seconds)


def strength(spectrum, freqs, hz):
    near = np.abs(freqs - hz) <= PITCH_TOLERANCE_HZ
    return spectrum[near].max()


# A square wave's third harmonic is a third of its note, about 10 dB down, where a
# sine's is only what the jack's filter and the line-in add.
THIRD_HARMONIC = 3


def check_purity(samples, expected, min_db):
    """Prints, for each side, how far its third harmonic is below its note, which shows
    a sine rather than a square wave. Returns the number of sides under `min_db`."""
    failures = 0
    for channel, name in enumerate(SIDES):
        data = samples[:, channel] - samples[:, channel].mean()
        spectrum = np.abs(np.fft.rfft(data * np.hanning(len(data))))
        freqs = np.fft.rfftfreq(len(data), 1 / SAMPLE_RATE)
        note = strength(spectrum, freqs, expected[name])
        harmonic = strength(spectrum, freqs, expected[name] * THIRD_HARMONIC)
        below = DECIBELS * np.log10(note / harmonic)
        good = below >= min_db
        failures += not good
        print(f"{name}: third harmonic {below:.0f} dB below the note: {'ok' if good else 'FAIL, not a sine'}")
    return failures


def check_pitches(samples, expected):
    """Prints, for each side, whether its strongest pitch is the expected one, and well
    above the other side's. `expected` maps "left" and "right" to Hz. Returns the
    number of sides that failed."""
    failures = 0
    for channel, name in enumerate(SIDES):
        data = samples[:, channel] - samples[:, channel].mean()
        spectrum = np.abs(np.fft.rfft(data * np.hanning(len(data))))
        freqs = np.fft.rfftfreq(len(data), 1 / SAMPLE_RATE)
        peak = freqs[spectrum.argmax()]
        other = expected[SIDES[1 - channel]]
        separation = DECIBELS * np.log10(strength(spectrum, freqs, expected[name]) / strength(spectrum, freqs, other))
        good = abs(peak - expected[name]) <= PITCH_TOLERANCE_HZ and separation >= MIN_SEPARATION_DB
        failures += not good
        print(f"{name}: expected {expected[name]:.1f} Hz, strongest {peak:.1f} Hz, "
              f"{separation:.0f} dB above the other side's pitch: {'ok' if good else 'FAIL'}")
    return failures
