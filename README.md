# Kitsune OS

The purpose of Kistune OS project is to help myself and others learn about low level programming and computer science.

It's built on the Raspberry Pi because it's cheap and simple platform.

## Install required tools

You're going to need the standard build tools for the target. The target prefix is `arm-none-eabi` so the standard build tools will have the same prefix such as `arm-none-eabi-gcc`, `arm-none-eabi-ld`, etc.

You'll also need `imagemagick` which is used to convert the logo in the `image` directory into raw bytes so they can be easily rendered to the framebuffer.

## Compiling the kernel with gcc

Once the above tools are installed simply run the following from the root directory of the project:

```
make
```

## Comping the kernel in Docker

Alternatively, you can build the kernel in Docker by running:

```
docker build --tag build-kitsune .
docker run -v "$(pwd)":/kitsune-os build-kitsune
```

# Running on QEMU

## Running Kitsune

Once you have `qemu-system-arm` installed you can run:

- ```make qemu```

... to build and run the `kitsune-qemu.elf` file in qemu.

## Running with the Serial Client

- Open a command line terminal in the `./tools/serial-client` directory

- Run `yarn` to install the `npm` packages

- Run `yarn tcp-mode` to start the serial client

- In the root directory, run `make qemu-tcp` to run Kitsune OS in QEMU with a TCP connection to the serial client on port `8081`

# Running on Raspberry Pi 4

- Build the Kitsune SD Card image by running `./tools/build-sd-card-image` (you may need to use `sudo` so the script can run the `losetup` and `parted` tools)

- Connect and empty SD Card to your workstation

- <span style="color: red;">**WARNING: BE CAREFUL HERE SO YOU DON'T OVERWRITE THE WRONG DISK**</span>

- Run `dd if=kitsune-sd-card.img of=/dev/${SD_CARD_DEVICE}`

- Install the SD card in the Raspberry Pi 4 and connect the power

**Note: USB Keyboard support is not yet supported. In order to interact with Kitsune OS you can communicate with it via the primary serial (uart) device on pins GPIO 14 (pin 8) and GPIO 15 (pin 10)**

## Running the Serial Client

- Open a command line terminal in the `./tools/serial-client` directory

- Run `yarn` to install the `npm` packages

- Update the `package.json` and replace `/dev/ttyACM0` with the name of your serial device that is connected to the Raspberry Pi

- Run `yarn serial-mode` to start the serial client

- Power on the Raspberry Pi 4

# Running the Serial Client Web UI

- Open a command line terminal in the `./tools/serial-client-web` directory

- Run `yarn` to install the `npm` packages

- Run `yarn start` to start the react app

**Note: Make sure the serial-client is running first**

# CREDITS

<img alt="Mascot" src="./image/mascot.png" width="150px">

Glorin Art (aka "GloristicArt" on Etsy)

https://www.etsy.com/people/duhruqyp

https://www.etsy.com/shop/GloristicArt

