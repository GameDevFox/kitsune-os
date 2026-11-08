# Hardware notes: Raspberry Pi 4

What we've measured on the real board, and what we've only read about. Everything marked measured was done on 2026-11-05 with a J-Link V12, jumper wires and the 2022 firmware on the SD card. It's one board and small samples, so treat the numbers as a guide. The scripts are in `tools/jtag/experiments/`.

## This board

Asked through the firmware's mailbox with `tools/jtag/experiments/framebuffer.py`.

| | |
| --- | --- |
| Revision | `0xc03112`, which decodes as a Pi 4B with 4 GB, revision 1.2 |
| CPU's share of RAM | `0x00000000`-`0x3b400000`, 948 MiB |
| GPU's share of RAM | `0x3b400000`-`0x40000000`, 76 MiB |
| Display | 1920x1080 at 32 bits per pixel |

These cover only the first GB. The rest of the 4 GB sits higher, and isn't in the mailbox's answer.

**The framebuffer is ordinary RAM.** The old kernel's `0x3e3cf000` falls inside the GPU's share of it. There's no separate video memory: the display hardware reads that RAM by itself, so a write needs no further step to appear. Stripes painted there over JTAG showed up straight, at the expected rows of a 1920-pixel-wide display, even under a kernel that never asked for a framebuffer. That address isn't guaranteed, so the kernel should ask the firmware.

The HDMI capture card is an MCT J5 (`0711:0a04`), which does 1080p Motion-JPEG at up to 60 fps. Through the server at 30 fps, it uses about 2 MB/s per viewer. It shows everything one pixel right of where it was drawn, leaving a dark line down the left edge. That threw off exact matching until `tools/screen_text.py` allowed for it. Its colors aren't exact either: pure white reads about 210, black about 17, and bright red about 219. So use QEMU for exact pixels. It also compresses to JPEG, so zooming in shows blurred edges and blocky noise that aren't in the kernel's framebuffer: the 4-color logo seems to gain grays and tints.

**Don't draw for long between serial checks, with the MMU off.** The UART's buffer overflows after about 1.4 ms at 115200 baud, by our estimate. A character takes much less than that, but clearing a text row in one go, about 58,000 writes, lost bytes: that's why rows are cleared a scanline at a time.

The mailbox works over JTAG with the core halted, with a physical address and no `0xC0000000` bus alias. Pixels are stored as blue, green, red, alpha.

## Transfer speed

Measured with 256 KiB of random data, written to RAM and read back intact, except at 20 MHz.

| JTAG clock | Write | Read |
| --- | --- | --- |
| 1 MHz | 66 KiB/s | 58 KiB/s |
| 2 MHz | 120 KiB/s | 94 KiB/s |
| 4 MHz | 199 KiB/s | 141 KiB/s |
| 8 MHz (our setting) | 313 KiB/s | 187 KiB/s |
| 12 MHz | 350 KiB/s | 204 KiB/s |
| 20 MHz | Failed, and left JTAG needing a power cycle | |

For comparison, UART at 115200 baud is about 11 KiB/s by calculation, as each byte takes 10 bits.

Measured through the server, with the kernel echoing each byte it polls: 65,536 random bytes came back intact in 6.6 s, about 10 KiB/s. So echoing keeps up with the line. A kernel doing slower work between polls could still lose bytes.

`cargo pi` takes about 0.45 s with no code change and 0.8 s with a recompile, for an 11 KB kernel.

## OpenOCD on this chip

- `verify_image` always fails, as it needs a checksum routine this target doesn't have. Use `dump_image` and compare the file.
- `read_memory` with a width of 8 runs at about 0.4 KiB/s. Width 32 reaches about 180 KiB/s.
- `load_image` reports success in its reply. `verify_image` and failures can reply with an empty string, so an empty reply doesn't mean success.
- `target/bcm2711.cfg` sets its own adapter speed, so a speed set before sourcing it is ignored.
- After GDB detaches, the core stays halted unless the config resumes it on `gdb-detach`. Ours does.
- A running OpenOCD reconnects by itself after a normal reboot or power cycle. After the debug path wedged (see "What a bad address can do"), it stayed stuck even once the Pi had been power-cycled, until OpenOCD itself was restarted.
- **It doesn't recover from the J-Link being unplugged,** even once it's back: it logs `LIBUSB_ERROR_NO_DEVICE` forever, and `halt` silently leaves the core running, so memory reads fail with "not halted". Restart OpenOCD. Seen 2026-11-07, after the user rewired.
- Over the TCL port, `mdw` replies with an empty string, as it prints to the log. `read_memory` returns the values.
- 32-bit kernels can be debugged too: it reads their registers and memory correctly, and GDB switches to ARM once given the 32-bit symbol file.

## Restarting the Pi

| | Runs firmware | Resets CPU state | Resets timers | Clears RAM |
| --- | --- | --- | --- | --- |
| `cargo pi` | No | No | No | No |
| Watchdog over JTAG | Yes | Yes | Yes | **No** |
| Power cycle | Yes | Yes | Not measured | Not measured |

### The watchdog reset

With core 0 halted, using `mww`:

```
mww 0xfe100024 0x5a00000a    # PM_WDOG: password + 10 ticks of about 16 us
mww 0xfe10001c 0x5a000020    # PM_RSTC: password + full reset
```

The values come from Linux's watchdog driver, as Broadcom doesn't publicly document this block. The card's kernel prints about 7 seconds later.

For several seconds after that, halting works but memory access over JTAG returns nothing. It then starts working, about 16 seconds after the reset in one run. The cause isn't known, so retry.

### What survives it

Measured by planting values, resetting once, and comparing.

| What | Result |
| --- | --- |
| RAM at `0x200000`, `0x8000000`, `0x10000000`, `0x20000000`, `0x30000000` | Survived, 16/16 words each |
| RAM at `0x1000000` | Overwritten during boot, by something unknown |
| System timer count | Restarted |
| System timer compare 1 and 3 | Read 0 |
| ARM timer load and reload | Read 0 |

The registers that read 0 may have been cleared by either the reset or the firmware. The test can't tell which.

**So never assume memory is zero, even after a reboot.** A forgotten initialization can quietly see the previous boot's data, and such a bug would disappear after a power cycle. The upside is that a kernel could leave a crash log in RAM to read after a reboot, as Linux's `pstore` does.

### What a bad address can do

Wild writes aren't only a kernel problem. A test build wrote pixels to `0xFE3CF000`, in the peripheral range, and that wedged the whole debug path: OpenOCD reported `Invalid ACK (0)`, core 0 wouldn't halt, and so neither `cargo pi` nor the watchdog could reach it. **Only a power cycle recovers from this**, as the watchdog reset needs JTAG, and OpenOCD must then be restarted too. It's the one case that needs hands or a remote power plug. Be careful with any mutation or test that changes an address or a stride, as it can do the same.

## What the firmware leaves us

- **Entry:** a 64-bit kernel is loaded at `0x80000` with `arm_64bit=1`. The entry exception level hasn't been read on hardware; we expect EL2. We reach EL1 either way.
- **UART:** with `enable_uart=1` and `dtoverlay=disable-bt`, the PL011 is on GPIO 14 and 15 at 115200 baud. The baud divisors read 26 and 3, which fit a 48 MHz clock. Our kernel doesn't set them.
- **JTAG:** `enable_jtag_gpio=1` puts GPIO 22 to 27 into Alt4. See the Raspberry Pi `config.txt` docs for the pin table.
- **Device tree:** it passes a pointer in `x0`, `0x2EFF2E00`, the same place the old 32-bit kernel found it in `r2`. Read on 2026-11-07 from a real boot: 53,684 bytes, version 17. `boot.s` saves a non-zero `x0` at `0x14000000`, which `link.ld` names `__device_tree_slot`, and `cargo pi` starts the kernel with `x0` at 0, so a JTAG run finds the tree the last real boot saved. Before any real boot of this kernel, the slot held leftovers, `0x3eb99318deecbb99`, which the kernel rejects by checking the range and the tree's magic number. QEMU starts the kernel with `x0` at `0x100`, which isn't a tree.
  - The tree agrees with what the kernel has by hand: memory `0x0` to `0x3B400000`, where the CPU's share ends, then `0x40000000` to `0xFC000000`, so this Pi has 4 GB and the kernel uses less than 1 GB of it. The GIC-400 is at `0x40041000` on `/soc`, whose `ranges` put it at `0xFF841000` for the CPU, the old project's `GIC_BASE`. `/soc`'s `dma-ranges` give `0xC0000000` as the DMA's view of RAM, as the sound's DMA uses.
  - Key `9` prints the whole tree over serial in `dtc`'s source form: 2,637 lines, about 65 KB, identical to `dtc -I dtb -O dts` on a copy dumped over JTAG, apart from the labels `dtc` rebuilds from `/__symbols__`.
  - The label `gicv2` is in `/__symbols__`, not `/aliases`, which holds others such as `serial0`.
- **Framebuffer:** the old kernel's hardcoded `0x3e3cf000` works on this firmware and display.
- **Power-up noise:** the UART shows a few stray bytes at power-on and power-off, such as `\x00\x00`. Anything parsing the line must tolerate them.

## Real hardware compared with QEMU

| | QEMU `raspi4b` | Real Pi 4 |
| --- | --- | --- |
| Entry level | EL3 | Expected EL2, unconfirmed |
| Registers at fault | Mostly zeros | Random-looking values in `x18`-`x29` |
| Alignment fault with the MMU off | Yes, same code `0x96000021` | Yes |
| Boot time to first output | Immediate | About 7 s |

## How fast memory is, with and without the MMU

Time to fill 1920x1080 at 32 bits, 8.3 MB, best of 3. From `tools/jtag/experiments/fill/run.py`.

| MMU | Memory type | Target | Method | Time | Rate | Fills per second |
| --- | --- | --- | --- | --- | --- | --- |
| Off | Device | Framebuffer | 4 bytes per write | 244.0 ms | 33 MB/s | 4 |
| Off | Device | Framebuffer | 16 bytes per write | 61.4 ms | 135 MB/s | 16 |
| Off | Device | Plain RAM | 4 bytes per write | 244.2 ms | 33 MB/s | 4 |
| Off | Device | Plain RAM | 16 bytes per write | 120.9 ms | 68 MB/s | 8 |
| On | Non-cacheable | Framebuffer | 4 bytes per write | 7.9 ms | 1,048 MB/s | 126 |
| On | Non-cacheable | Framebuffer | 16 bytes per write | 6.2 ms | 1,343 MB/s | 162 |
| On | Write-back | Plain RAM | 16 bytes per write | 6.3 ms | 1,324 MB/s | 160 |
| On | Write-back | Plain RAM | `dc zva` by line | 6.6 ms | 1,259 MB/s | 152 |
| On | Write-back + clean | Framebuffer | 16 bytes per write | 7.0 ms | 1,178 MB/s | 142 |
| On | Write-back + clean | Framebuffer | `dc zva` + clean | 7.1 ms | 1,168 MB/s | 141 |

**The MMU being off is what makes the kernel slow, not memory bandwidth.** With it off, all memory is treated as device memory: each write is sent on its own, with no combining or caching. Turning it on made the same write about 31 times faster, even with the framebuffer non-cacheable.

The write-back fill does reach the screen: the color it painted showed up in the capture. The kernel still runs with the MMU off, so every memory access it makes pays this cost. The odd gap between the two 16-byte device rows, 135 and 68 MB/s, is unexplained.

### Store shapes, and why write-back doesn't help a fill

Measured on 2026-11-05 with `tools/jtag/experiments/stores/run.py`: filling the 8.3 MB framebuffer, 5 runs per case, 3 full runs, median speeds. Every run used the 54 MHz generic timer, which matched the system timer. AArch64 has no `STM`, so "11 regs" and "13 regs" are 32-bit `STP W` and `STR W` stores with those chunk sizes, the nearest thing.

| Stores | Chunk | Device (MMU off) | Device, misaligned | Non-cacheable, fastest | Write-back |
| --- | --- | --- | --- | --- | --- |
| `STR W` | 4 | 33 MB/s | | 1,041 to 1,048 MB/s | 1,067 MB/s |
| `STR X` | 8 | 67 MB/s | | About 1,325 MB/s | 1,325 MB/s |
| `STP X` | 32 | 131 MB/s | 126 at +8 | About 1,325 MB/s | About 1,300 MB/s |
| "11 regs" | 44 | 133 MB/s | 129 at +4 | About 1,325 MB/s | 1,325 MB/s |
| `STP X` | 48 | 196 MB/s | 160 at +8 | About 1,200 MB/s | 1,325 MB/s |
| "13 regs" | 52 | 150 MB/s | 142 at +4 | About 1,325 MB/s | 1,325 MB/s |
| `STP X` | 64 | 251 MB/s | 185 at +8 | About 1,300 MB/s | 1,199 MB/s |
| `STP Q` | 64 | 250 MB/s | Faults at +8 | About 1,300 MB/s | About 1,275 MB/s |
| `DC ZVA` | 64 | Faults | | 1,325 MB/s at best | About 1,300 MB/s |

- **Only device memory cares about store shape.** It gave the same speeds to within 1 MB/s on every run. A 64-byte aligned chunk is fastest, and being 8 bytes off costs about a quarter of that. An `STP X` faults at an offset of 4, as device memory requires alignment to the store's size.
- **With the MMU on, nearly everything hits about 1.3 GB/s.** Chunk size and misalignment stop mattering. Only single 32-bit stores are slower. The small differences between other rows aren't meaningful.
- **Non-cacheable runs flip between about 1,325 and 641 MB/s,** in the same case from one full run to the next, so it isn't caused by the store shape. The cause is unknown. Plain RAM and the framebuffer both reached the same top speed.
- **The cache works, but fills don't get its benefit.** On a 16 KB buffer, non-cacheable against write-back reads were 367 and 4,757 MB/s, and writes of one word per line were 160 and 1,161 MB/s. `SMPEN` is on. But repeatedly filling that same 16 KB in write-back reached only 1,450 MB/s, and 256 KB about 1,530 MB/s.
- **A fill seems to leave nothing dirty.** Cleaning the whole buffer costs 46,662 ticks (0.86 ms) with nothing written and 46,702 after a fill, when writing out dirty lines would take far longer. The likely explanation is the A72's write streaming, where whole-line writes skip allocating in the cache, but we haven't proved that. One outlier may be related: "13 regs" at +4 on 16 KB gave about 1,900 MB/s on both runs that measured it.
- **So the clean to hand a frame to the GPU is cheap.** All of these numbers are at the firmware's default 600 MHz, as the next section explains.

### The CPU clock

**The firmware leaves the CPU at 600 MHz, and it can run at 1,500 MHz.** Reading the rate and asking for the firmware's maximum over the mailbox is all it takes, with no `config.txt` change. Measured on 2026-11-05 with `tools/jtag/experiments/clock/run.py`: one run, going low, high, then low again, where both low phases agreed. Its tags are `0x00030047` for the measured rate, `0x00030004` for the maximum, and `0x00038002` to set a rate.

| Test | 600 MHz | 1,500 MHz | Gain |
| --- | --- | --- | --- |
| Measured core clock | 200 MHz | 500 MHz | The firmware raises it too |
| Spin loop, MMU on | 300 M/s | 750 M/s | 2.5x |
| Fill 8.3 MB, 64-byte chunks, write-back | 1,325 MB/s | 3,128 MB/s | 2.4x |
| Fill 8.3 MB, "13 regs", write-back | 1,325 MB/s | 3,128 MB/s | 2.4x |
| Fill 8.3 MB, `STR W`, write-back | 1,070 MB/s | 2,708 MB/s | 2.5x |
| Fill 8.3 MB, 64-byte chunks, device | 251 MB/s | 339 MB/s | 1.35x |
| Fill 8.3 MB, "13 regs", device | 142 MB/s | 184 MB/s | 1.3x |
| Fill 8.3 MB, `STR W`, device | 33 MB/s | 53 MB/s | 1.6x |
| Fill 16 KB, already read into the cache | 4,755 MB/s | 11,888 MB/s | 2.5x |
| Read 16 KB, write-back | 4,757 MB/s | 11,894 MB/s | 2.5x |
| Read 16 KB, non-cacheable | 367 MB/s | 628 MB/s | 1.7x |
| Write one word per line, non-cacheable | 160 MB/s | 173 MB/s | 1.1x |
| Clean 8.3 MB, write-back | 9,595 MB/s | 23,967 MB/s | 2.5x |

- **A full-screen write-back fill takes about 2.7 ms at 1,500 MHz,** which is about 370 fills per second, against 6.3 ms at 600 MHz.
- **We can't say whether the CPU or the core clock set the old limit,** as they rose together.
- **The earlier 1.3 GB/s wall was the clock, not memory.** SDRAM's rate wasn't reported: the tag we tried returned 0.
- **Reading a buffer first makes filling it 3.3 times faster.** Filling 16 KB gave about 1,450 MB/s when it hadn't been read, and 4,755 MB/s when it had, both at 600 MHz. That fits write streaming: whole-line writes skip the cache only if the lines aren't already in it. It still isn't direct proof.
- **Non-cacheable fills still drop to a slow mode,** which hardly changes with the clock: about 641 MB/s at 600 MHz and 691 MB/s at 1,500 MHz, against 3,127 MB/s at its best. Write-back never showed it. So far that favors write-back plus a clean for the framebuffer.
- **Temperature went from 51 to 54 C, with no throttling flags,** but this was seconds of load. Sustained load and cooling aren't tested, and neither is HDMI at the raised core clock, as nothing checked the screen during the high phases.

**The spin loop may matter more than any of this.** With the MMU off, it ran at 8 M/s at 600 MHz and 14 M/s at 1,500 MHz, against 300 and 750 M/s with the MMU on. The kernel runs with SCTLR's instruction cache bit clear, so each fetch may be going to memory. The architecture allows the instruction cache on with the MMU off, which would be a one-bit test, not yet tried.

### How long drawing Aki takes

Measured on 2026-11-05 with `tools/jtag/experiments/draw/run.py`, on the real Pi, with the MMU off as in the real kernel: median of 5 draws of her body and mouth onto a cleared screen, going low, high, then low again, where both low phases agreed to within 1 ms.

| Version | Stored size | 600 MHz | 1,500 MHz | Gain |
| --- | --- | --- | --- | --- |
| Original, full color | 1,486 KB | 508 ms | 312 ms | 1.6x |
| x1 | 427 KB | 371 ms | 233 ms | 1.6x |
| x2 | 384 KB | 363 ms | 228 ms | 1.6x |
| x3 | 339 KB | 342 ms | 214 ms | 1.6x |
| Clearing the screen | | 244 ms | 155 ms | 1.6x |

**Stored size barely affects the time.** x3 is 21% smaller than x1 but only 8% faster, and the original holds 3.5 times the data of x1 but takes 37% longer. See the next section for how the time divides between decoding and writing.

## Speeding up the kernel

One change at a time, simplest first, each measured on the real Pi with `tools/jtag/experiments/draw/run.py`, which copies the current kernel, so it measures whatever that kernel does. Each row adds to the one above. The user confirmed on 2026-11-05 that the PC-98 images and keys work on the real Pi.

| Step | Clear | Original | x1 | x2 | x3 |
| --- | --- | --- | --- | --- | --- |
| Baseline: 600 MHz, MMU and caches off | 244 ms | 508 ms | 371 ms | 363 ms | 342 ms |
| Kernel raises the clock to 1,500 MHz at boot | 155 ms | 290 ms | 212 ms | 204 ms | 191 ms |
| Instruction cache on, measured by switching it in one boot | 48 ms | 183 ms | 148 ms | 129 ms | 114 ms |
| Instruction cache on in the real kernel, from `boot.s` | 48 ms | 184 ms | 155 ms | 135 ms | 119 ms |
| MMU on, data cache off | 3 ms | 42 ms | 70 ms | 59 ms | 52 ms |
| Data cache on, framebuffer and mailbox uncached | 3 ms | 5 ms | 4 ms | 4 ms | 4 ms |

The last row is confirmed from a real boot, with the images correct on the capture. Its numbers are near the 1 ms resolution of this test, so differences between them mean little. Clearing didn't change, as the framebuffer is uncached. The screen's memory is reached at `0x7E3CF000`, its normal address plus `0x40000000`.

**With the MMU on, the original became the fastest to draw,** and x1 the slowest, the reverse of before. We didn't look into why. x1's dithering leaves few runs, so most of its pixels are raw indexes, each decoded and drawn on its own, but that isn't measured. The experiments `stores` and `clock` were written for a kernel that starts with the MMU off, so their MMU-off results no longer apply if rerun.

The switching rows are from `tools/jtag/experiments/icache/run.py`, in one boot going off, on, then off, where both off phases agreed to within 3 ms. The last row is from `draw/run.py`, whose three phases agreed to within 1 ms. It differs slightly from the row above, for reasons we didn't look into. Two oddities:

- **Clearing with the cache off was bimodal,** with a fastest time of 155 ms but a median of 308 ms in both off phases. The table uses the fastest. It was a steady 155 ms in the earlier draw test, and the cause is unknown.
- **The same draws with the cache off differ between the two experiments,** such as x1 at 212 ms here and 233 ms in the draw test. The cause is unknown, though the two kernels have different code layouts.

Decoding measured by drawing past the screen's right edge, which does all of the decoding and writes nothing, at 1,500 MHz:

| Version | Whole draw, cache off | Decoding, cache off | Whole draw, cache on | Decoding, cache on |
| --- | --- | --- | --- | --- |
| Original | 290 ms | 186 ms | 183 ms | 90 ms |
| x1 | 212 ms | 133 ms | 148 ms | 75 ms |
| x2 | 204 ms | 124 ms | 129 ms | 68 ms |
| x3 | 191 ms | 112 ms | 114 ms | 60 ms |

- **Decoding is the larger part, not pixel writes.** It's about 60% of a draw with the cache off, and about 50% with it on, for x1 through x3. That's a correction to our earlier guess. These times include the mouth for a whole draw but not for decoding, which is a small difference.
- **The cache helps code more than it helps drawing.** A spin loop went from 14 to 187 M/s, 13 times faster, and clearing from 155 to 48 ms, as that loop's own instructions were being fetched from memory. A whole draw gained 1.4 to 1.7 times.
- **The spin loop still has room:** 187 M/s against 750 M/s with the MMU on, for reasons not yet known.
- **1,500 MHz held up.** After two minutes of constant drawing, the temperature rose from 50 to 56 C with no throttling flags, and HDMI stayed up.

### Stale code after `cargo pi`

**With the instruction cache on, `cargo pi` runs the previous kernel's code unless the caches are cleaned first.** JTAG writes go straight to RAM, without the caches noticing. `tools/jtag/experiments/stale/run.py` loads two kernels that differ by one number in their code, back and forth. With the cleaning at the start of `boot.s`, all 6 loads printed the right number. Without it, all 3 switches to the other kernel printed the previous number, and the kernel crashed when old and new code got mixed.

The cleaning flushes the kernel's memory with `dc civac`, for L2's sake, then uses `ic iallu`. It reads no literals, as a stale copy would look for them in the wrong place. A stale copy of it still works, as any version clears the same memory, but a kernel without it can't protect the next one: loads after a kernel built without the cleaning printed nothing at all until a reboot. We didn't test whether L2 really holds stale code, or whether `ic iallu` alone would do.

`boot.s` also turns the MMU and data cache off first, as zeroing `.bss` erases the page tables, then cleans every data cache by set and way, as dirty lines from the old kernel could overwrite the new one when evicted. Results with the data cache on:

| Removed from `boot.s` | Right |
| --- | --- |
| Nothing | 6 of 6 |
| All of it | 0 of 6, each crashing |
| Cleaning by address only | 6 of 6 |
| Turning the MMU off, and cleaning by set and way | 6 of 6 |

Each half is enough alone in this test, as each ends with its own `ic iallu`. So nothing here shows that turning the MMU off, or cleaning by set and way, is needed: the first probably survives because the TLB keeps translations alive while the tables are zeroed, and dirty lines would only cause trouble at some later eviction, which this test wouldn't see. Both are kept, as the alternative relies on what the TLB and cache happen to hold.

### A cube drawn by the CPU

Key `5` spins a 400x400 cube in `kernel/src/cube.rs`, with fixed-point math, drawing into a cached buffer and copying that to the uncached screen. On the Pi, with the MMU and caches on, at 1.5 GHz, one frame took 1,269 us on average over 546 frames, covering the clear, drawing and copy. That's room for about 790 frames per second, though it runs at 60. It's the baseline for the GPU.

## The GPU

**The V3D 3D core works from bare metal, with no firmware or Linux involved beyond what's already running.** On 2026-11-06 it drew a 400x400 triangle in 194 us, on the first run, and then the cube below. `kernel/src/board/raspi4/gpu/` has the code, and its `CREDITS` the license. Broadcom publishes no manual, so this comes from [a bare-metal example](https://github.com/Random06457/rpi4-gpu-bare-metal-examples) (MIT), Linux's `drivers/gpu/drm/v3d`, and Mesa's `src/broadcom`. OpenGL and Vulkan aren't available, as they're Mesa's work on top of Linux's driver, so everything is control lists and shader code.

- **Power:** set bit 6 of `PM_GRAFX` (`0xFE10010C`, with the password), then let the two bridges at `0xFEC1100C` and `0xFEC11008` run. Don't read GPU registers before that, as it could hang the bus.
- **Identity:** the hub reads `0x42554856` (`VHUB`), and the core `0x04443356` (`V3D` and version 4).
- **Memory:** the GPU uses physical addresses, with its own MMU left off, and can't see into the CPU's cache. So its buffers sit at `0x10000000`, and the CPU only reaches them through the uncached view.
- **Jobs:** binning runs a list by writing its start and end to `0x160` and `0x168` in the core's registers, with the tile memory set first, and rendering uses `0x164` and `0x16C`. The counters at `0x134` and `0x138` move when each finishes.
- **Shaders:** currently compiled shader code from that example, as there's no assembler here.
- **Screen:** it renders straight into the framebuffer, as a raster image at the framebuffer's stride with red and blue swapped. Though it works in whole 64-pixel tiles, it wrote nothing past the 400x400 image.
- **Depth:** the depth buffer only exists in the tile buffer, so it needs no memory or a store. Turning the test off on a copy of the kernel clearly broke the picture, with back faces over front ones, so it's really working.

| Key `5` and `6` cube, 400x400, at 1.5 GHz | Time per frame |
| --- | --- |
| CPU, drawing to a cached buffer, then copying | 1,269 us |
| GPU, with the CPU only transforming 36 vertices | 229 us |

These are each frame's whole cost, over 546 and 594 frames. There's no vsync, so tearing is possible. A longer run of the GPU cube, 14,418 frames over 4 minutes at 60 per second, gave the same 229 us with no timeouts.

## Interrupts

Since 2026-11-07 the kernel takes interrupts, through the GIC-400 (`kernel/src/gic.rs`), with the core's virtual timer ticking 100 times a second as the only source so far (`kernel/src/interrupts.rs`). At boot it counts the ticks over one second.

- **Both came from the device tree:** the GIC's distributor at `0xFF841000` and CPU interface at `0xFF842000`, through the label `gicv2` and `/soc`'s `ranges`, and the virtual timer's interrupt, 27, the third entry in `/timer`'s `interrupts` (PPI 11, plus 16). The board's constants are only the fallback, which QEMU uses, as it passes no tree.
- **It counted exactly 100 ticks in 1 s,** after `cargo pi`, in five reloads in a row over a kernel already ticking, and after a real boot from the card through the EL2 path. QEMU counted 99, with its timer at 62.5 MHz; the Pi's runs at 54 MHz, though `/timer` says `arm,cpu-registers-not-fw-configured`.
- **The firmware already puts every interrupt in group 1,** so the kernel, running non-secure, only enables that group, and sets nothing else: priority mask, then each interrupt's priority and enable bit.
- **The virtual timer, not the physical one,** as EL1 can always use it, while the physical one needs EL2's permission, which `boot.s` doesn't give.
- **A reload is the dangerous moment.** A running kernel's `cpsr` read `0x80000345`, with IRQs on, and a tick taken between `cargo pi`'s resume and `_start` masking them would jump through the old vectors into new code. So `cargo pi` writes `cpsr` with all four mask bits set first, which reads back as written, and `boot.s` masks them and stops the virtual timer as its first instructions.
- **Sound, serial uploads and the reboot command all still worked with the tick running,** and so did every QEMU image check.
- **Drivers register their own handlers,** by interrupt number, in `interrupts::register`, so the dispatcher knows nothing about them.

### Timers

Since 2026-11-07, `kernel/src/time.rs` gives uptime from the ARM generic counter, and alarms from the system timer's compare channels (`board/raspi4/system_timer.rs`), whose address and interrupts come from the device tree's `system_timer` label: shared interrupts 64 to 67, numbered 96 to 99 by the GIC.

- **A shared interrupt must be sent to a core.** The alarm's match bit set, and the GIC showed interrupt 97 enabled and pending, but its target byte (`GICD_ITARGETSR`) was `0x00`, so it went nowhere. Private interrupts, like the timer tick, have read-only targets. `Gic::enable` now targets a shared interrupt at the core enabling it, read from the first target byte, which gives a core its own bit. The old project's `route_irq_to_cpus` did the same.
- **The alarm then arrived 0 us after it was due,** at the system timer's 1 us resolution, after `cargo pi` and after a real boot.
- **The ARM counter starts when the ARM cores do, not at reset.** It ran 6,965,125 us behind the system timer in one run and 6,965,126 us in another, so the two agree in rate. After a real boot they read about 1.05 s and 8.0 s. That gap is the firmware's time before starting the kernel.
- **The GPU uses system timer channels 0 and 2,** as far as we know from other code; the datasheet doesn't say. The kernel uses channel 1.
- **QEMU never delivers the alarm:** its match bit sets, but no interrupt arrives, even with the target set. The tick works there.
- **A JTAG reload keeps the old kernel's GIC and devices running.** After the heartbeat existed, a reloaded kernel hung just after raising the clock: the old heartbeat's alarm on channel 3 was still enabled in the GIC, so it arrived the moment interrupts were let in, before anything had registered for it. With nothing to clear the timer's match, it arrived again forever. `Gic::start` now disables and clears every line, and an interrupt with no handler is disabled rather than ended and left to repeat. Three reloads in a row over a running heartbeat then booted cleanly.

### Small devices

Proofs of concept from 2026-11-07, each using only the device tree and its own registers. They're optional, so without a tree, as in QEMU, they're skipped rather than given fallback addresses.

- **Cycle counter** (`arch/aarch64/cycles.rs`): 14,998,654 cycles in 10 ms against the ARM counter, so 1,499 MHz, where the firmware set 1,500. QEMU's reads 999 MHz.
- **Temperature** (`board/raspi4/thermal.rs`, `temperature.rs`): the sensor at offset `0x200` of the AVS monitor (`avs_monitor`, `0xFD5D2000` for the CPU), with the conversion from `/thermal-zones/cpu-thermal`'s `coefficients`, a slope of -487 and an offset of 410,040 thousandths of a degree. It read 56.478 C against the firmware's 56.442 C, and 55.017 C against 54.981 C after a reboot. QEMU has no sensor, and reading where it would be is a data abort.
- **Random numbers** (`board/raspi4/rng.rs`, `random.rs`): the RNG200 at `0xFE104000`, set up as Linux's `bcm2711_rng200` does. 1,024 words took about 62 ms, about 530 kbit/s, with 49.3% to 50.2% of bits set across runs and no repeated words.
- **Activity LED** (`board/raspi4/gpio.rs`, `heartbeat.rs`): GPIO 42, active high, from `/leds/led-act`. It blinks once a second from system timer channel 3's alarm, which it owns, so it doesn't depend on the tick. Read over JTAG with the core halted, its level alternated, and the alarm moved on by 500,000 us each time. Making it an output changes only its own bits in `GPFSEL4`, which also holds the sound's pins 40 and 41.
- **Watchdog** (`board/raspi4/watchdog.rs`, `watchdog.rs`): in the power manager (`watchdog`, `0xFE100000`), with the registers in "The watchdog reset" above. Its countdown is 20 bits at 65,536 ticks a second, so at most about 16 s, and it keeps counting while JTAG has the core halted, so nothing leaves it armed. The check arms 100 ms and feeds every 20 ms for 300 ms: it never fell below 80 ms, and cancelled. A copy that stopped feeding reset the board within the check, and the card's kernel booted 7 s later. Reboots are essential, for `cargo upload reboot`, so they keep a fallback address, but the check needs the tree: **QEMU resets the moment it's armed**, ignoring the countdown, so it rebooted forever. **The firmware clears `PM_RSTS`**: it read 0 after both a reboot and a starved countdown, so its "had a watchdog reset" bit (`0x40` in Linux) can't tell the kernel why it restarted.
- **Reading memory over JTAG needs the core halted.** Unhalted reads fail, and a script that parses their output can mistake the failure for a value.

## An unexplained crash after a watchdog reboot

On 2026-11-05 the card's kernel crashed twice during experiment runs, each time just after a watchdog reset: a data abort on a store to `0x49a` at `0x81738`, in `fill_rect` drawing the square, inlined into `animation::run`. Both crashes showed the same registers, apart from `x0` and `x1`, including the same row and column, so the same frame. The register holding the pitch, `x22`, was 1 rather than 7680, which made the address garbage.

A third crash came a little later, 25 s after a reboot, just after one JTAG halt, read and resume: an unrecognized exception at `0x200`, `ESR` `0x2000000`. OpenOCD's own log reported `pc: 0x200` at that halt. Its scrollback showed 3 of 190 halts at that address, and all the rest in the kernel.

**Still unexplained.** JTAG halting is the main suspect, because every crash came just after a halt and resume, and the first two match the steady 13.1 s at which `tools/jtag/experiments/reset.py` resumes. But it's rare: `tools/jtag/experiments/crash/run.py` then found 0 crashes in 12 rounds, 4 each with no JTAG after the reboot, with the reset script's pattern, and with a late halt. So those rounds don't separate the cases, and they can't clear the kernel. `0x200` is also the exact offset of the synchronous exception vector, which may or may not mean anything. None of the crashes touched the experiment kernels that ran after them.

A fourth came soon after, in the card's kernel, just after `draw/run.py`'s closing watchdog reset: an unrecognized exception at `0x83e90`, which is past that build's code. So it's less rare than those 12 rounds suggested. The card's kernel is older than the instruction cache change, and leaves the cache off. Crashes in test kernels with the cache cleaning removed don't count, as those are expected.

Two more came later in the card's kernel with the instruction cache on, so that isn't the difference: at `0x83a9c` accessing `0xd2867fe0d51ce07f`, with a second exception landing while the first printed, and at `0x825e4` accessing `0x91`. Both came just after the reboot that follows the `stale` test's round with the cleaning removed, but a later run of that round was followed by a clean reboot, so that may be a coincidence.

Later on 2026-11-06, a build with the MMU and caches on crashed the same way, on a store from the same square-drawing loop to `0x2F3097E63AA`, again just after `reset.py`'s closing resume. **The row in `x21` and `x11` was 758, and `x8` was `0x31E`, in this build and the first two crashes, so the same frame in very different kernels.** Nothing in a kernel ties it to that frame, but `reset.py` resuming about 13.1 s after a reset does. The pitch's register was wrong each time, so JTAG's halt and resume corrupting registers is now the leading explanation. It's still not proven.

## How Linux's console scrolls so smoothly

From reading the Raspberry Pi Linux tree, not from measuring it. The main reason is the MMU and caches, but it also avoids work:

- **It only draws characters that changed.** `fbcon_redraw` (`drivers/video/fbdev/core/fbcon.c`) keeps the screen as text, compares each cell with the one scrolling into its place, and draws only the ones that differ. Most of a console row is blank, so a scroll touches far less than the whole screen.
- **It can draw in cached memory.** `drm_fbdev_dma.c` can use a cached shadow buffer and copy over only the damaged area, later, from a worker (`drm_fb_helper_damage_work`). Several scrolls can be merged into one copy. Whether vc4 on a given setup takes this path depends on the framebuffer having a `dirty` function.
- **It scrolls whole rows,** so it isn't really smooth pixel-by-pixel motion.

## Flipping buffers and syncing with the display

### What we measured

On 2026-11-05, with `tools/jtag/experiments/vsync/run.py`, resetting before each test. The screen was sampled through the capture card at about 18 frames per second.

| Test | Result |
| --- | --- |
| HVS line counter, channel 1 | **Works.** Status `0xFE400058`, low 12 bits count 0-1080, wrapping every 16,664-16,668 us. Read-only |
| `SET_VSYNC`, `0x0004800e` | **Doesn't work here.** No reply within 1 s, and a late reply can confuse the next mailbox call |
| SMI interrupt | **Not seen** in 0.5 s. `SMICS` stayed `0x54000000` |
| One flip with virtual offset | Accepted every time, in about 180 us, and the offset reads back correctly |
| Time for a flip to appear | **About 1.3 s** after the call, in each of 6 flips |
| Flips every 250 ms, synced or not | The color changed only about every 1.25 s. No lost signal in 20 s |
| Flips every 20 ms, 60 in a row | 32 of 60 calls reported failure, for an unknown reason |
| Flips every 27 ms, unsynced | **HDMI signal lost** after about 6 s |
| Flips every frame, synced to the line counter | **HDMI signal lost** after about 8 s |

**So don't use virtual offset for animation.** It's far too slow to appear, and sustained flipping kills the HDMI signal until a reset. Syncing doesn't prevent that. `cargo pi` can't restore the signal, but the watchdog reset does (`tools/jtag/experiments/reset.py`).

The card appears to show lost signal as pure black (about 0 in every channel, and a 41,109-byte JPEG both times we checked its size), whereas real black reads about `0x11`.

We never captured a torn frame. With flips this slow to take effect that means little, and the 18 fps sampling could miss one anyway. The 1.25 s pattern is measured, but its cause is a guess: it looks as though the firmware only picks up the offset occasionally.

### From reading the Linux source

From the Raspberry Pi Linux tree, branch `rpi-6.12.y`.

| Way to sync | What Linux shows | Where |
| --- | --- | --- |
| Mailbox tag `SET_VSYNC`, `0x0004800e` | `FBIO_WAITFORVSYNC` sends this with a dummy word and waits for the reply, so the call presumably blocks until vsync. It isn't in the firmware's published tag list. | `drivers/video/fbdev/bcm2708_fb.c`, `include/soc/bcm2835/raspberrypi-firmware.h` |
| SMI interrupt, GIC SPI 112 | The firmware delivers vblank through the otherwise unrelated SMI block at bus address `0x7e600000`, normally `0xfe600000` for the CPU. Pending in `SMICS` bits 9-11, cleared by writing 0. Newer firmware marks `SMIDSW0` with `0xabcd0000` and sets bit 0 for display 0. Whether it fires without the firmware KMS driver's setup is unknown. | `drivers/gpu/drm/vc4/vc4_firmware_kms.c`, `arch/arm/boot/dts/broadcom/bcm2711-rpi-ds.dtsi` |
| HVS status | `SCALER_DISPSTATX` has a line counter (bits 0-11), so the scan position may be readable directly. | `drivers/gpu/drm/vc4/vc4_regs.h` |

Flipping:

- **Virtual offset,** tag `0x00048009`, is how the legacy framebuffer driver pans. It sends the whole configuration again each time, including `ALLOCATE`, rather than just the offset. Nothing there says when the offset takes effect.
- **Changing the buffer pointer takes effect mid-frame in native vc4.** `vc4_plane_async_set_fb` says it applies "immediately (well, once the FIFO needs refilling)". That's for programming the HVS directly, a different path from the firmware's virtual offset.

The vc4 drivers program the HVS directly, which the firmware otherwise owns on our setup, so treat those registers as read-only.

## Reading the SD card

Measured on 2026-11-05, from a watchdog reboot, with the MMU off. `tools/jtag/experiments/sd/` holds the code: `peek.py` shows the controller's state, `read_block.py` reads over JTAG, and `run.py` runs `experiment.rs` from the CPU. All of it only sends read commands.

**The firmware leaves everything ready, so reading a block needs no setup.** Its last command was CMD12, ending a multi-block read. The clock is on, the bus is 4-bit and high speed, and the card is selected and idle. Sending CMD17 straight away worked on the first try, over JTAG and from the CPU. The result was checked against the card's real volume ID, found by following its partition table to block 8192.

To read a block, all through 32-bit accesses at `EMMC2` (`0xFE340000`), as Linux only uses those on this chip:

1. Write all ones to `0x30` to clear the interrupt status.
2. Write `count << 16 | 512` to `0x04`, the block number to `0x08`, and the command to `0x0C`.
3. Wait for bit 0 of `0x30`. Then, for each block, wait for bit 5 and read 128 words from `0x20`. Finally wait for bit 1. Bit 15 means an error. Write a bit back to `0x30` to clear it.

| Commands (`0x0C`) | Speed |
| --- | --- |
| One block at a time, `0x113A0010` (CMD17) | About 1 MB/s, as each command costs about 1 ms |
| Many at once, `0x123A0036` (CMD18 with automatic CMD12) | 8.5 to 11.7 MB/s |

Repeated reads of 1 MB and 16 MB gave identical hashes, and both commands agreed. A second 16 MB read was faster than the first; a cache in the card might explain that, but we didn't check. The speed fits a 25 MHz clock on 4 data lines, a limit of 12.5 MB/s, assuming the capabilities register's 100 MHz base clock and the divider of 4 are right. Aki at 427 KB would take about 50 ms.

`kernel/src/board/raspi4/sd.rs` has this as `read_blocks`, using the single-block command for one block and the multi-block one otherwise.

**This has limits:**

- **It only works after the real firmware.** In QEMU the same code times out on the first command, with a fake card attached, so testing there needs a real initialization (CMD0, CMD8, ACMD41, CMD2, CMD3, CMD7).
- **Block numbers assume an SDHC or larger card.** Older cards address by byte. This card uses blocks, and we didn't read the type from it.
- **Not tested:** recovering from an error, which needs a controller reset, and whether the firmware ever touches the card after boot. Only a watchdog reboot was tested, not a power cycle.

### Writing

**Writing works with no setup too.** `tools/jtag/experiments/sd_write/run.py`, on 2026-11-05, wrote a pattern to block 8000, in the unused gap before the first partition, read it back correctly, then restored the block's original zeros and read those back correctly. It uses CMD24 (`0x183A0000` in `0x0C`), waits for bit 4 of `0x30`, writes 128 words to `0x20`, then waits for bit 1.

Speeds from `tools/jtag/experiments/sd_speed/run.py`, in the same gap, with the MMU and data cache on: median of 5, with the same area rewritten each time. Multi-block writes use CMD25 with automatic CMD12 (`0x193A0026`).

| Operation | Median | Speed |
| --- | --- | --- |
| Write 1 block | 2.3 ms | 218 KiB/s |
| Write 8 blocks | 2.6 ms | 1.5 MiB/s |
| Write 64 blocks | 5.0 ms | 6.5 MiB/s |
| Write 1 MB | 112 ms, once 217 ms | 9.1 MiB/s |
| Read 1 MB | 88 ms | 11.6 MiB/s |

The 1 MB read back correctly, and the area was restored and checked. The very first write in the earlier test took 105 ms, so a card can pause now and then. These repeat a small area of a card that has plenty free, so a nearly full card or a long write could be slower. A read straight after a write could come from the card's own buffer, so this doesn't show data surviving a power cycle.

### Writing through the filesystem

`card::overwrite` replaces a file's contents through the `fat` crate, and only for the files in its `WRITABLE_FILES`. `tools/jtag/experiments/sd_overwrite/run.py`, on 2026-11-06, wrote 300 KB to the 512 KB `update-test.bin` on the real card and read all of it back in 4,036 ms, with `kitsune.img` and `config.txt` refused. After a watchdog reboot, a separate kernel found the same contents. A watchdog reboot leaves the card powered, so a power cycle hasn't tested this yet, and neither has reading the file on a PC.

### Uploads over serial

Tested on 2026-11-06 on `update-test.bin`, with a 72 KB file: 7.7 s to send, about 9 KB/s, and 12 s in all with the write and read back. With every 5th chunk damaged it took 49 s, with 74 resends, and with every 7th dropped 37 s, with 49, each ending correct. A 600 KB file was refused, and a wrong CRC left the kernel saying it hadn't touched the card, which still needs checking on a PC. The animation stops during an upload, as serial's FIFO overflows in about 3 ms at this rate.

### Installing a kernel without touching the card

`cargo pi --install` loads a kernel into RAM over JTAG, with a second copy in the upload memory, starts it, then sends one packet asking it to check that copy's CRC and write it to `kitsune-next.img`. On 2026-11-06, the whole thing took 7.8 s, with about 1.7 s to load, 1.5 s for the new kernel to start, and 3.9 s to write and read back, plus any compiling. It needs no cache handling, as booting cleans every cache, and the CRC would catch it if not. A kernel with a marker installed this way showed it after a real reboot from the card, and the normal one, installed the same way, took it away.

`cargo upload kernel` does the same over serial, in about 13 s for this size, for when JTAG isn't connected. `cargo upload reboot` has the kernel restart itself, in about 7 s, with no JTAG.

**Trial boots work on this Pi.** With the firmware's one-shot flag set, using tag `0x00038064` with value 1, then `0x00030048`, and then the watchdog, it boots what `tryboot.txt` names. The next reboot goes back to `config.txt`'s kernel. A marker kernel confirmed both directions. A deliberately broken one hasn't been tried, as an occasional card swap to recover is fine.

The self-reboots gave no crashes in 8 tries. That's more evidence against JTAG's halt and resume, but not proof, as the crash was intermittent.

### What else is on the card

**The card holds old data outside its partition table, so don't assume free space is blank.** Its one partition ends at block 532480, 260 MiB in, of a 7.97 GB card. Starting at exactly that block there's an old filesystem from a previous use of the card, with no partition entry. Other spots hold scattered old data too, such as a run of `0xFF` at 2, 4 and 6 GiB, and something signed `LPKSHHRH` at 3 GiB, 4 MiB in, which we haven't identified.

A survey of 6 MB at each whole GiB from 2 to 7 found only 5 and 7 GiB blank, so images went at 5 GiB, block 10485760.

### Loading images from raw blocks

The raw area holds the same bytes as the image region in RAM, with the same table and offsets, so `Assets::get` copies an image's blocks to its offset in RAM the first time it's asked for, and hands it out from there afterward. If the card has no valid table, the kernel uses what's in RAM, which is how QEMU works, as it can't read the card. The card comes first, as a watchdog reboot doesn't clear RAM, so old images could look loaded.

`tools/jtag/experiments/sd_images/run.py` copied the images to the card in 1.38 s for 5.3 MB, read them back correctly, then booted a kernel that first zeroed the image region. All the images drew correctly on the capture, by shape, and 46 ms covers the first draw of everything at boot. First reads took 37 ms for x1's 427 KB, 34 ms for x2's 384 KB, 30 ms for x3's 339 KB, and 129 ms for the original's 1,486 KB. Pressing a key again read nothing. The raw area isn't kept up to date by anything, so it has to be rewritten when images change.

**Rewriting it, from 2026-11-07, is `tools/write_images.py`,** as the card now boots the kernel without images, so a reboot no longer puts them in RAM for the copier. It loads `target/assets.bin` into RAM over JTAG with core 0 halted, in 17 s, and reads it back, in 30 s. It then runs the copier, which wrote 5.3 MB in 0.5 to 1.1 s, and reads RAM back again, as its own check only compares the card with RAM. **The copier mustn't look up images at startup:** the card comes first, so that would read the card's old images into RAM over the new ones. On a blank area, as in the first run, it couldn't happen. The first rewrite replaced x3 with x4, and both checks matched.

### How long booting takes

From a watchdog reset to the card's kernel first printing, with no JTAG involved, took 8.16 s in all 3 runs of `tools/jtag/experiments/boot_time.py`, with a 7 MB `kitsune-next.img`. The kernel takes under 30 ms of it, so the rest is the firmware. The kernel now prints the timer when it starts, which measures this directly on a real boot.

**Shrinking that file to 37 KB, with images read from raw blocks, took it to 6.89 s,** and 6.97 s until the kernel is ready for input, against 8.19 s. The kernel's own timer agreed, at 6,883 to 6,906 ms. So the firmware spent about 1.27 s on the extra 7 MB, about 5.5 MB/s, half the speed of the kernel's polled reads, and the kernel's first draw now costs 46 ms rather than 4 ms. The other 6.9 s is firmware work we haven't looked into.

## Audio from the headphone jack

The jack's audio comes from PWM1, on GPIO 40 and 41, and the firmware already sets those pins to it (`GPFSEL4` read `0x64`). Linux doesn't drive it directly: its `bcm2835-audio` driver asks the GPU firmware through VCHIQ, which we avoid. Here the PWM is written directly.

**A square wave needs no code at all,** only register writes, as the PWM repeats a pulse by itself. `tools/jtag/experiments/tone.py` does it over JTAG and records this PC's line-in:

1. Stop the PWM clock: `CM_PWMCTL` (`0xFE1010A0`) = `0x5A000001`, oscillator as source. Its divisor may only change while it's stopped and not busy.
2. `CM_PWMDIV` (`0xFE1010A4`) = `0x5A000000 | 1000 << 12`, so 54 MHz / 1000 = 54 kHz.
3. `CM_PWMCTL` = `0x5A000011` to enable it.
4. For each channel, a range in `RNG1` or `RNG2` (`0xFE20C810`, `0xFE20C820`), half of it in `DAT1` or `DAT2`, then `CTL` (`0xFE20C800`) = `0x8181`: both channels on, in mark-space mode.

Measured 2026-11-07, with a 3.5 mm cable into the line-in of the PC's onboard USB audio:

- **The pitch was exact:** a range of 123 gave 439.0 Hz, which also confirms the oscillator is 54 MHz. The third harmonic at 1,317 Hz showed it's a square wave.
- **Channel 1, on GPIO 40, is the right side,** and channel 2, on GPIO 41, the left. Each channel's pitch was 71 dB above the other's, so they're cleanly separate.
- **The level was −3.5 dBFS,** peaking at 80% of full scale, so it doesn't clip at the PC's current line-in gain. Silence was −61 dBFS, mostly mains hum at 30 to 90 Hz.
- Turning `CTL` back to 0 silenced it.

The clock-manager registers for the PWM aren't in the BCM2711 datasheet; their addresses and password come from other code.

### Samples from the kernel

Key `7` plays a test tone through the PWM's FIFO, in `kernel/src/sound.rs` and `board/raspi4/audio.rs`: a 27 MHz clock (54 MHz / 2) and a range of 562 give 48,042.7 samples per second, which the kernel calls 48,042, with about 9 bits per sample. The channels take turns reading the shared FIFO, channel 1 (the right side) first. This section's measurements were made with the CPU polling until the FIFO had room, which paused the screen; sound now goes through DMA, below. `tools/check_tone.py` presses the key and checks the recording. Measured 2026-11-07:

- **Both sides were right:** 880.1 Hz on the left and 659.8 Hz on the right, each 47 to 55 dB above the other's pitch, at about a fifth of full scale, as set.
- **About 510 ms of steady tone, with no gaps,** for a 500 ms tone, in 10 ms steps.
- **The tone became a sine wave later on 2026-11-07,** middle C (261.63 Hz) on the right and the G above it (392 Hz) on the left, from the same fixed-point cosine table as the turning logo (`kernel/src/trig.rs`), faded in and out over 20 ms. It measured 261.9 and 392.0 Hz, 61 to 66 dB apart, and each side's third harmonic was 58 to 59 dB below its note, where the square wave's was 9 to 10 dB, so the jack's filter adds little distortion. `tools/check_tone.py` checks that too, with a 30 dB bar.
- **Switching the PWM on and off pops,** as the output jumps between 0 V and the middle level that is silence. Without easing, the start clipped the PC's input at 32,745 of 32,767. A 50 ms ramp each way brought the start down to about 7,700 and the end from 21,906 to about 9,300. The line-in's own filter still drifts for a few hundred ms after a tone.
- **QEMU has no PWM:** writing to it is a data abort that stops the kernel. The kernel only plays if the firmware connected GPIO 40 and 41 to the PWM, which QEMU doesn't, and `tools/check_images.py qemu` presses `7` to check that.

### A recorded clip

Key `8` plays a clip from RAM at `0x12000000`, which `tools/voice.py` cuts from one of Momo's voice lines and loads over JTAG, or, if RAM holds none, the clip in the images' last slot, which comes off the card: her 187 KB line read in 17 ms, measured 2026-11-07, and played at 0.997 with no clip in RAM. Clips are signed 16-bit mono at the kernel's 48,042 samples per second, scaled on both sides to full scale either side of silence. Measured 2026-11-07 with `tools/check_voice.py` on "Okay, let's do this!", 2.0 s:

- **It matched the original at 0.998 on each side,** lined up and compared between 300 and 3,000 Hz. The speed was exact: the clip's last third landed 0 samples from where it should.
- **Compared across all frequencies, it only reached 0.34,** as the start-up pop and the line-in settling after it are far louder below 300 Hz than the voice. So the check uses the speech band.
- **The recording was quiet,** peaking at 11% of full scale, which would leave her voice about 6 of the PWM's 9 bits. `voice.py` now scales each clip to peak at 90%.
- **The check can fail:** pressing a key that plays nothing gave 0.057, and expecting the clip 2% faster gave 0.361.
- The kernel's rate is 27 MHz / 562 = 48,042.7, rounded down. A clip at 48,043 was refused, as the kernel checks the rate.

### Through DMA, in the background

Since 2026-11-07, the kernel writes a whole sound, ramps included, into a buffer at `0x13000000`, through the uncached view, as right and left words in turn. DMA channel 5 then copies it into PWM1's FIFO, paced by DREQ 1, and the kernel returns at once. Each frame, `Player::poll` checks whether it's done and turns the PWM off. There are no interrupts. The DMA uses the datasheet's legacy addresses: RAM through the window at `0xC0000000`, and the FIFO at `0x7E20C818`.

- **It played correctly:** Momo's line matched at 0.997 to 0.998, and the tone passed. `tools/check_voice.py` now also watches the screen: the orbiting square was in 8 different places in 8 frames while she spoke. A kernel that waited for the sound to finish froze it, in 2 places, and failed.
- **Preparing a 2 s clip took 583 us,** writing about 790 KB uncached.
- **DREQ 1 is shared between DSI0 and PWM1,** chosen by `DMA_CNTRL_MUX` in `PACTL_CS` (`0x7E204E00`), whose bits the datasheet doesn't give. The firmware leaves it reading `0x08800000`, and DREQ 1 paced the PWM without touching it, as in rpi4-osdev's part 11, which also plays PWM sound by DMA.
- **Linux's device tree leaves DMA channels 0, 2 and 4 to 10 to the ARM** (`brcm,dma-channel-mask = <0x07f5>`), so channel 5 avoids the firmware's. All 15 were enabled after boot (`0xFE007FF0` read `0x7FFF`).
- **The PWM needs time between setup steps.** Written back to back, its enable didn't take: the control register read `0x2121`, but the channels stayed idle, the DMA stopped after 10 words, and it started as soon as the register was written again by hand. Writing PWM registers before its clock ran set its bus error flag (`STA` bit 8). So the kernel waits for the clock's busy bit, then 2 ms after each step, as rpi4-osdev does. Polling had worked without this, perhaps because the enable was rewritten often enough by accident; not looked into.
- **Every sound ends with the PWM's gap flags set** (`STA` `0x32`), as the FIFO runs dry after the last sample, so they aren't reported as problems.
- **A trap while experimenting:** the kernel kept running between JTAG halts, and its `poll` turned the PWM's clock off, which made later hand-written tests fail in misleading ways. Keep the core halted for the whole of a hand-run test.

Longer or mixed sounds need a looping buffer that the kernel keeps topping up.

**The capture card has an audio input too,** so HDMI audio could be checked from this PC the same way.

## Which peripheral next

Our own rough judgment, for orientation. We haven't built any of these. The details are from memory unless linked or marked, so check them before starting one.

| | SD card | Ethernet | USB |
| --- | --- | --- | --- |
| Difficulty | **Lowest** | Middle | **By far the highest** |
| Hardware | EMMC2, SDHCI-style controller | GENET MAC plus BCM54213 PHY | VL805 xHCI chip behind PCIe |
| Public docs | SD and SDHCI specs | Little from Broadcom; read the Linux and U-Boot drivers | xHCI and USB specs, nothing for the Pi's PCIe glue |
| Needs DMA | No, polled reads work | Yes, descriptor rings | Yes, and many structures |
| Firmware has already set it up | Yes, it read the card to boot | Not that we know of | No, and needs a firmware request for the VL805 |
| In this QEMU build | Yes (`generic-sdhci`, `sd-card`) | Not visible | Not this path; only `dwc2-usb` |
| First useful result | Read one block | Send and receive a raw frame | Find a device on a hub |
| Then still needs | A FAT filesystem | ARP, IP and UDP, such as the `smoltcp` crate | Enumeration, hubs and class drivers |
| Risk | Writing can corrupt the boot card, so start read-only | Low | Low |

The USB-C port is a separate DWC2 controller, which QEMU does emulate, and may be simpler than the VL805 path, but it isn't what the USB-A ports use. As far as we know, DWC2 has little public documentation.

The latest QEMU docs list GENET and PCIe for `raspi4b`, but `info qtree` on the installed 11.1.1 shows neither. A newer build may help with those.

Sources: [QEMU Raspberry Pi docs](https://www.qemu.org/docs/master/system/arm/raspi.html), [FreeBSD's BCM2838 xHCI driver](https://src.rcs.uwaterloo.ca/xref/freebsd-src/sys/arm/broadcom/bcm2835/bcm2838_xhci.c), [Linux patch for the VL805 firmware load](https://lkml.iu.edu/hypermail/linux/kernel/2005.0/04848.html).
