# Kitsune OS (next)

A from-scratch kernel in Rust and assembly, for the Raspberry Pi 4. This is the current version of Kitsune OS. The C kernel in the parent directory is the original, now deprecated.

On boot it draws Aki with the Kitsune logo turning over her outstretched hand, in perspective, while a square circles the logo through the colors of the rainbow. Text runs down the left side, and a mouse cursor moves over everything. Keys do the rest:

| Key | What it does |
| --- | --- |
| `1` | Draw Aki's original picture |
| `2`, `3`, `4` | Draw her in PC-98 style, at x1, x2 and x4 dots |
| `5` | Spin a cube in the logo's place, drawn by the CPU |
| `6` | The same cube, drawn by the GPU |
| `7` | Play a test tone from the headphone jack: middle C on the right, the G above it on the left |
| `8` | Play a recorded voice clip, if one was built in (see below) |
| `9` | Print the device tree over serial |
| `0` | Clear the screen |
| `c` | Run the drivers' self-checks |

Underneath: interrupts and a timer tick, the MMU and caches, an SD card driver that reads images from raw blocks, a FAT reader for updating the kernel over serial, HDMI drawing through the firmware's framebuffer, PWM sound by DMA, the V3D GPU, and drivers for the watchdog, thermal sensor and random number generator. There's no floating point in the kernel, so the logo's turn and the cube use fixed-point math from tables built at compile time.

- [`docs/design.md`](docs/design.md): how the kernel and tools are structured, the choices made, and how it's tested on the real board
- [`docs/hardware-notes.md`](docs/hardware-notes.md): measurements of the real board, debugger quirks, and how the peripherals behave

## 1. Install the tools

You need [Rust](https://rustup.rs/), Python 3 and ImageMagick (`magick`). Then, from this folder:

```bash
rustup target add aarch64-unknown-none-softfloat
rustup component add llvm-tools
cargo install cargo-binutils
```

It's built with Rust 1.95. For QEMU, install `qemu-system-aarch64`. The checks in `tools/` also use Python's `numpy`, and a voice clip needs `ffmpeg`.

## 2. Build it

```bash
scripts/build-image
```

This makes `target/kitsune-next.img`: the kernel, with Aki's pictures and the logo bundled after it.

## 3. Try it in QEMU (optional)

```bash
qemu-system-aarch64 -M raspi4b -serial stdio -kernel target/kitsune-next.img
```

A window shows the screen, and the terminal shows the kernel's text. QEMU has no sound, and its timing isn't the real board's. (`cargo run` also boots it, but only the kernel, without its pictures, and with no window: it's for quick text-only checks.)

## 4. Run it on a Raspberry Pi 4

You need a Pi 4, an SD card, and a monitor on HDMI that can show 1920x1080.

1. **Prepare the card.** Write Raspberry Pi OS Lite to it with [Raspberry Pi Imager](https://www.raspberrypi.com/software/). Kitsune doesn't use the OS, but the card's boot partition then has the Pi's firmware and device trees, which it needs. (It's tested with the firmware from the 2022-09-22 release.)
2. **Copy the kernel** `target/kitsune-next.img` onto the card's boot partition, the small FAT one.
3. **Edit `config.txt`** on that partition, and add these lines at the end:

   ```
   [all]
   arm_64bit=1
   enable_uart=1
   dtoverlay=disable-bt
   kernel=kitsune-next.img
   ```

   `arm_64bit` boots it as a 64-bit kernel, and `kernel` names it. The other two put the Pi's main serial port on GPIO 14 and 15, where Kitsune expects it.
4. **Put the card in the Pi and power it on.** In a few seconds you should see Aki, the turning logo and the rainbow square, with text down the left. If the screen stays black, check those `config.txt` lines first.

### Pressing keys

There's no USB keyboard support yet. Keys reach the kernel over the Pi's serial port, so you need a USB serial adapter with 3.3 V levels. Wire it like this:

- the adapter's RX to GPIO 14, pin 8;
- its TX to GPIO 15, pin 10;
- its ground to a ground pin, such as pin 6.

It runs at 115200 baud.

Keys travel as small framed packets, defined in [`protocol/`](protocol/), not as plain typed characters: a serial terminal shows the kernel's text, but typing in it only echoes. The server here sends real keys:

```bash
cargo server --device /dev/ttyUSB0      # your adapter's device
```

Then open `http://localhost:8766/` in a browser, and press keys on that page. It works without the rest of the setup below, though its video area stays empty without a capture card. `cargo uart --seconds 5` shows the kernel's recent text from another terminal, and `curl --data-binary 1 localhost:8766/key` sends a key from a script.

### A voice clip

The voice clip isn't in this repo. To build one in, make `next/voice.local.json`, which git ignores, naming a recording on your machine, for example `{"recording": "~/sounds/hello.opus"}`. `tools/voice.py` describes the format. Then `scripts/build-image` cuts it and stores it with the pictures, where key `8` finds it.

## Going further: the development setup

Day to day, it's developed with more hardware, and the tools in `tools/` and `scripts/` assume it:

- **A JTAG debugger** (a J-Link, with `enable_jtag_gpio=1` in `config.txt`) and OpenOCD, run with `openocd -f tools/jtag/pi4.cfg`. Then `cargo pi` loads a new kernel into the running Pi in about a second, with no card swap, and `cargo pi --install` also writes it to the card.
- **An HDMI capture card**, which `cargo server` streams to its web page, so the screen can be watched, and checked, from anywhere on the network.
- **The headphone jack wired to the PC's line-in**, for the sound checks.

`scripts/card.py` swaps kernels on a card in a reader, and expects one particular card, so change its volume ID to match yours.

## Tests

```bash
cargo clippy                 # lints, including the kernel's ban on floating point
cargo test -p device_tree -p fat -p pi -p protocol -p server -p uart -p upload --target host-tuple
python3 tools/check_images.py qemu    # after build-image: compares QEMU's screen with the expected pictures
```

The other checks in `tools/` need the real board and the setup above.
