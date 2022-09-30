# Kitsune OS

The purpose of Kistune OS project is to help myself and others learn about low level programming and computer science.

It's built on the Raspberry Pi because it's cheap and easy platform.

## Compiling the kernel

You're going to need the standard build tools for the target. The target prefix is `arm-none-eabi` so the standard build tools will have the same prefix such as `arm-none-eabi-gcc`, `arm-none-eabi-ld`, etc.

You'll also need `imagemagick` which is used to convert the logo in the `image` directory into raw bytes so they can be easily rendered to the framebuffer.

Once the tools are installed simply run:

`make`

... from the root directory of the project.

## Running on QEMU

If you have `qemu-system-arm` installed you can run:

```make qemu```

... to build and run the `kitsune-qemu.elf` file in qemu.

## Running on Raspberry Pi 4

- Install the latest version of Raspberry PI OS (sometimes also known as Rasbian) on and SD card

- Mount the `/boot` partition and copy the `kistune.img` binary file from into build into that directory

- Update the `/boot/config.txt` file and change the `kernel=???` line to `kernel=kitsune.img`

- Unmount the `/boot` directory from the SD card

- Install the SD card in the Raspberry Pi 4 and connect the power

**Note: USB Keyboard support is not yet supported. In order to interact with Kitsune OS you can communicate with it via the primary serial (uart) device on pins GPIO 14 (pin 8) and GPIO 15 (pin 10)**

## Running the Serial Client with QEMU

- Open a command line terminal in the `./tools/serial-client` directory

- Run `yarn` to install the `npm` packages

- Run `yarn tcp-mode` to start the serial client

- In the root directory, run `make qemu-tcp` to run Kitsune OS in QEMU with a TCP connection to the serial client on port `8081`

## Running the Serial Client with Raspberry Pi 4 hardware

- Open a command line terminal in the `./tools/serial-client` directory

- Run `yarn` to install the `npm` packages

- Update the `package.json` and replace `/dev/ttyACM0` with the name of your serial device that is connected to the Raspberry Pi

- Run `yarn serial-mode` to start the serial client

- Run `picocom --quiet --baud 115200 --noreset --nolock /dev/${YOUR_SERIAL_DEVICE}` to set the baudrate of the serial device to `115200`

- Power on the Raspberry Pi 4

## Running the Serial Client Web UI

- Open a command line terminal in the `./tools/serial-client-web` directory

- Run `yarn` to install the `npm` packages

- Run `yarn start` to start the react app
