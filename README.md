# Kitsune OS

The purpose of the Kitsune OS project is to help myself and others learn about low level programming and computer science.

It's built on the Raspberry Pi because it's a cheap and simple platform.

## Two versions

This repo holds two versions of Kitsune OS. **If you're new here, use the new one.**

| | Where | What it is |
| --- | --- | --- |
| **New: Kitsune OS next** | [`next/`](next/README.md) | The current version. A from-scratch 64-bit kernel in Rust and assembly for the Raspberry Pi 4. It draws Aki with the Kitsune logo turning over her hand, spins a cube on the GPU, and plays sound. |
| **Old: the original (deprecated)** | everything else at this level | The first version: a 32-bit kernel in C, with a web UI for inspecting it over serial. It's frozen and kept for reference, and isn't maintained. |

A later release will reorganize the folders so that the new version sits at the top. Until then, the old version's files stay where they are.

## Running the new version

The steps are in [`next/README.md`](next/README.md). In short, from `next/`:

1. Install Rust with the `aarch64-unknown-none-softfloat` target, `cargo-binutils`, Python 3 and ImageMagick.
2. Build it with `scripts/build-image`, which makes `target/kitsune-next.img`.
3. Try it in QEMU with `qemu-system-aarch64 -M raspi4b -serial stdio -kernel target/kitsune-next.img`, or put it on a Raspberry Pi 4's SD card, with a few lines added to its `config.txt`.
4. To press keys on the Pi, connect a USB serial adapter and run `cargo server`, then use its web page.

# The old version (deprecated)

Everything from here on is about the original C kernel. It isn't maintained, and its build hasn't been checked since the folders were last reorganized, so expect to fix things to run it.

- [Kernel architecture](docs/architecture.md)
- [Serial protocol and APIs](docs/serial-protocol.md)

## Install required tools

You're going to need the standard build tools for the target. The target prefix is `arm-none-eabi` so the standard build tools will have the same prefix such as `arm-none-eabi-gcc`, `arm-none-eabi-ld`, etc.

You'll also need `imagemagick`, `bc` and `xxd`, which are used to convert the images in `kernel/image` into raw bytes so they can be easily rendered to the framebuffer.

## Compiling the kernel with gcc

Once the above tools are installed simply run the following from the root directory of the project:

```
make -C kernel
```

This produces `kitsune-qemu.elf` for QEMU, and `kitsune.elf` and `kitsune.img` for the Raspberry Pi 4.

## Compiling the kernel in Docker

**Note: the `Dockerfile` hasn't been updated for the current folder structure and needs fixing first.**

## Running on QEMU

### Running Kitsune

Once you have `qemu-system-arm` installed you can run:

```
make -C kernel qemu
```

... to build and run the `kitsune-qemu.elf` file in QEMU, with its serial port on your terminal.

### Running with the Serial Client

From the root directory of the project:

```
yarn
yarn workspace @kitsune-os/serial-client tcp-mode
```

Then in another terminal, run Kitsune OS in QEMU with a TCP connection to the serial client on port `8081`:

```
make -C kernel qemu-tcp
```

The serial client serves a REST API on port `8080` and a GraphQL API on port `4000` once QEMU has connected.

## Running on Raspberry Pi 4

**Note: `build-sd-card-image` hasn't been updated for the current folder structure and needs fixing first.**

- Build the Kitsune SD Card image by running `./build-sd-card-image` (you may need to use `sudo` so the script can run the `losetup` and `parted` tools)

- Connect an empty SD Card to your workstation

- <span style="color: red;">**WARNING: BE CAREFUL HERE SO YOU DON'T OVERWRITE THE WRONG DISK**</span>

- Run `dd if=kitsune-sd-card.img of=/dev/${SD_CARD_DEVICE}`

- Install the SD card in the Raspberry Pi 4 and connect the power

**Note: USB Keyboard is not yet supported. In order to interact with Kitsune OS you can communicate with it via the primary serial (uart) device on pins GPIO 14 (pin 8) and GPIO 15 (pin 10)**

### Running the Serial Client

- Update `serial-client/package.json` and replace `/dev/ttyACM0` with the name of your serial device that is connected to the Raspberry Pi

- From the root directory of the project, run `yarn` and then `yarn workspace @kitsune-os/serial-client serial-mode` to start the serial client

- Power on the Raspberry Pi 4

## Running the Serial Client Web UI

**Note: Make sure Kitsune OS and the serial client are running first**

Run the following from the root directory of the project:

```
yarn
yarn workspace @kitsune-os/common build
yarn workspace serial-client-web start
```

# CREDITS

<img alt="Aki" src="./kernel/image/aki/base.png" width="150px">

Aki is by ShizukoDesu (https://shizukodesu.carrd.co/), Momo's voice by MomoBunnyVA, and the mascot by Glorin Art. See [CREDITS.md](CREDITS.md) for everyone, and for the code this project borrows.
