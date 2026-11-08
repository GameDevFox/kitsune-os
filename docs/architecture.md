# Kernel architecture

Addresses come from `nm` on the `kitsune-qemu.elf` built 2024-08-03. The section order is fixed by `kernel/linker.ld`, but exact addresses shift with every build, so get current ones from `make -C kernel nm` or `GET /kernel-symbol`.

## Boot sequence

The firmware (or QEMU's `-kernel`) enters `_start` at `0x8000`, in `kernel/arch/arm/boot.S`:

1. Sets `sp` to `_start`, so the boot stack grows down from `0x8000`.
2. If `r2` is 0, replaces it with the embedded device tree. QEMU takes this path: at boot it prints `atags: 0x006d8038`, an address inside the kernel's own `.data`.
3. Pushes `r0-r2` for `main`.
4. Parks every core except core 0 in `wfi`.
5. Drops from HYP to SVC if it was entered in HYP.
6. Points VBAR at `exception_vector` and sets the UND, ABT, IRQ and FIQ stacks.
7. Enables the cycle counter, zeroes BSS, and runs `perf_test`.
8. Calls `main`.

`main` (`kernel/core/main.c`) then:

1. Calls `uart_init` and prints the banner and boot registers.
2. Builds the MMU translation table, without enabling the MMU.
3. Draws the curve, logo, Aki, the text and the clock.
4. Prints `READY`.
5. Enters `input_loop`, which never exits unless the `b` command runs. It alternates between redrawing the uptime clock and polling the UART.

There are no interrupts in normal operation: IRQs stay masked and everything is polled.

## Memory map

| Address | What |
| --- | --- |
| `0x40`-`0x5f` | `perf_test` scribbles its 8 cycle-count samples here |
| below `0x8000` | Boot stack and the main SVC stack, growing down |
| `0x8000` | `.text.boot`, the `__eden` code and data |
| next page | `.rodata.image`, all images and fonts. This is most of the kernel, about 6.8 MB |
| `0x6d3000` | `.text.exception_vector` |
| next pages | Exception stacks |
| `0x6d8000` | `__data_start`: `.data`, including the embedded device tree |
| next | `.rodata` |
| `0x6e7000` | `.bss` |
| `0x6e8000` | `.text`, deliberately last |
| `0x6eb000` | `__end` |
| `0x01000000` | MMU translation table (16 KB, written at boot) |

## `__eden` and flashing

`__eden` (`kernel/core/section.h`) places a function or variable in `.text.boot` at the bottom of the image. The linker order puts `.data`, `.rodata`, `.bss` and `.text` *after* it, and that order is the point.

The goal is to replace the running kernel over the serial port without touching the SD card. The `flash` GraphQL mutation sends the image from `__data_start` to the end through the `W` command. So everything `write_memory` needs while the overwrite is in progress must live below `__data_start`:

- `write_memory`
- `uart_getc`, `uart_getw`, `uart_putc`, `uart_puts`
- `mmio_read`, `mmio_write`
- all `sp_*` functions
- the `mmio_base` and `fb_base` variables (`fb_base` isn't used during a write, but is marked the same way)

**This is why a new dependency of `write_memory` must be `__eden` too.** The linker will not complain if it isn't; the kernel simply overwrites the code it is running.

The feature is an unfinished draft ("Rough draft for memory flashing", 2023). `flash` currently fails on a stale path, and nothing restarts the kernel after writing.

## Boards

`kernel/config/*.c` supplies three values, and the Makefile links one config per output:

| Config | `raspi` | `mmio_base` | `fb_base` | Built as |
| --- | --- | --- | --- | --- |
| `qemu-raspberry-pi-2b.c` | 2 | `0x3f000000` | `0x3c100000` | `kitsune-qemu.elf` |
| `raspberry-pi-3b.c` | 3 | `0x3f000000` | `0x3e402000` | **not linked by any target** |
| `raspberry-pi-4b.c` | 4 | `0xfe000000` | `0x3e3cf000` | `kitsune.elf`, `kitsune.img` |

- `raspi >= 3` makes `uart_init` ask the VideoCore mailbox to set the UART clock to 3 MHz.
- **The framebuffer is not requested from the firmware.** `fb_base` is a hardcoded address, and the mailbox is only used for the UART clock. Resolution is fixed at 1920x1080 at 32 bpp by `fb.h` and the QEMU flags. The Pi 4 value was confirmed on real hardware on 2026-11-05, with the 2022 firmware on the card, by the Aug 2024 `kitsune.img`. It may still depend on firmware version and display.
- Everything compiles with `-mcpu=cortex-a7` for all boards, in 32-bit mode.

## Exceptions

`kernel/arch/arm/vector.S` holds a VBAR table of 8 entries. Handlers print a banner over the UART and mostly return with `eret`.

- Data abort does not return to the faulting code. It jumps straight to `input_loop`, on the abort stack, in abort mode.
- The IRQ handler is written for the Pi 4 GIC, and its GIC acknowledge and end-of-interrupt accesses are commented out.

## Input and commands

`root_handler` receives each polled byte and passes it to the current `input_handler`. Command mode is the default. See `docs/serial-protocol.md`.

Commands that take arguments read them with blocking `uart_getc` calls inside the handler. While a command runs, the main loop (and the clock) stops.

## Live coprocessor access

`` ` `` and `~` (MRC and MCR) work by self-modifying code. `generate_mrc` assembles an instruction word, `call_instruction` writes it over `live_instruction` in `.text`, then calls `live_fn`, which runs it surrounded by barriers. `!` does the same with any word in `binary_entry`, so any single instruction can be run from the client.

This relies on `.text` being writable, which holds while the MMU is off.

## MMU state

The MMU is **off**, and no kernel code turns it on. On QEMU, SCTLR reads `0x00c50078`, so M, C and I are all clear.

`setup_mmu_translation_table` writes a flat 4 GB identity map at `0x01000000`: 256 x 16 repeated supersection entries of 16 MB each, full access, uncached. No code writes TTBR0, TTBCR, DACR or SCTLR.M. The apparent path to enabling it is by hand, through the Coproc Registers tab, whose register definitions (including `TTBCR`) were added around the same time. That is an inference, not something tested.

## Interrupts

`z`, `x`, `c` and `q` are experiments towards a timer interrupt. They use `GIC_BASE = 0xff840000`, a Pi 4 address, so they are not expected to work on QEMU's raspi2b. `cpsr_enable_irq` is commented out in `main`.

## Image pipeline

1. A PNG in `kernel/image/` is built by `kernel/image/Makefile`. `image-size-gen` writes an 8-byte header of little-endian width and height, and `magick` appends raw pixels with red and blue swapped.
2. `ld -r -b binary` wraps the `.data` file into a `.o` exposing `_binary_<path>_data_start`.
3. The linker script collects `image/*` into `.rodata.image`.
4. `draw_image` copies runs of pixels to the framebuffer. **Alpha is binary**: any pixel whose alpha isn't `0xff` is skipped.

Fonts are the same format: a sheet of 16 x 8 ASCII cells (`character-sheet-{base,micro,nano}.png`).

To add an image:

1. Put the PNG under `kernel/image/`.
2. Add its `.o` to `IMAGE_OBJS` in `kernel/Makefile`.
3. Add a `DRAW_IMAGE(name, x, y)` line in `fb.c` and a prototype in `fb.h`.
4. Bind a command in `command_handler`.
5. To expose it to the UI, add it to `images` in `serial-client/src/api.ts`.

## Device tree

`parse_device_tree` walks the FDT from the pointer in `r2` and prints it. The `dts/*.dtb` files are board blobs and the `.dtc` files are their source. Only `4-model-b.dtb` is linked, even into the raspi2b build, so on QEMU the `d` command prints a Pi 4 tree.
