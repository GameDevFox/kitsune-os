# Design notes

How the new kernel and its tools are put together, and why. For measurements of the hardware, see `hardware-notes.md`.

## Structure

| Path | What it knows |
| --- | --- |
| `kernel/src/main.rs`, `console.rs`, `text.rs`, `video.rs`, `animation.rs`, `image.rs`, `assets.rs` | Nothing about any particular CPU or board |
| `kernel/src/arch/aarch64/` | The CPU: boot, exception levels, vectors and the MMU |
| `kernel/src/board/raspi4/` | The board: addresses, the UART, the mailbox and the memory layout |
| `fat/` | Finds a file on FAT32 and overwrites its data in place, tested against real filesystems |
| `protocol/` | The packets sent over serial, shared by the kernel and server, and tested on the host |
| `server/` | Owns the serial port and capture card, and shares them |
| `tools/uart/`, `tools/pi/`, `tools/upload/` | Serial viewer, the build-load-run loop, and sending files to the card over serial |
| `tools/screen_text.py` | Reads on-screen text back from the capture |
| `tools/check_images.py` | Compares drawn images with their source files |
| `tools/page_test/` | Tests of the video page, in a headless browser |
| `tools/jtag/experiments/` | Scripts behind the measurements in `hardware-notes.md` |

## Portability

The aim is to start on the Pi 4 and later reach other Pi models, other architectures such as x86_64 and RISC-V, and perhaps some game consoles, without abstracting too early.

- **Rules from day one:** no hardware addresses outside `board/`, and no assembly outside `arch/`. These cost almost nothing, and they keep the seams clean.
- **No traits until a second port needs them.** `main.rs` picks its `arch` and `board` modules with `#[path]`, and the functions they export are the porting interface. A new port gets a compile error for each function it's missing, and that port is the right time to design any abstraction.
- **Decide early what's hard to change later,** such as giving a wire protocol explicit widths and endianness, not assuming a transport is a UART, and not assuming 64-bit pointers in portable code.

## Choices made

- **Rust and assembly, no C.** The old C kernel is frozen, and kept as a reference and a known-good baseline.
- **AArch64 on the Pi 4,** so that QEMU's `raspi4b` and the real board are the same machine.
- **Softfloat target, and no floats in the kernel.** Clippy rejects float math and the `f32` and `f64` types, and the target rejects FP instructions in assembly. So kernel math uses integers, and the square's orbit uses the Minsky circle algorithm: `x -= y >> 6; y += x >> 6`.
- **Boot drops to EL1,** whichever level the firmware starts at, so exception handling has only one set of registers to deal with.
- **The firmware's framebuffer comes from the mailbox,** not a hardcoded address.
- **One program owns each device.** Two readers of a serial port split its bytes, and a capture card can only be opened once, so `cargo server` holds both and shares them.
- **Drivers start as isolated, low-level proofs of concept.** Each should do its one job without depending on or interfering with the others, keeping today's interleaving to a minimum. The architecture that has systems working together comes afterward, designed deliberately rather than grown. The user's direction, 2026-11-07.
- **New drivers find their addresses in the device tree first,** with a hard-coded fallback for when there's no tree, as in QEMU or a `cargo pi` run before any real boot, but only for what the kernel can't do without, such as interrupts and timers. Optional devices, such as the temperature sensor, the random number generator and the LED, are skipped without a tree: QEMU lacks some of them, and touching a missing device is a data abort. That's a step toward one image for every machine. Drivers written before the tree reader keep their constants: the user decided on 2026-11-07 not to convert them, as they're proven on the board and a fallback would still be needed. `kernel/src/devices.rs` and the `device_tree` crate do the lookups.
- **Checks run on request, never at boot.** Quick iteration matters most, from the user on 2026-11-07, so a driver's proof goes in `kernel/src/checks.rs`, which `c` runs, rather than in the boot path. Together they took about 1.6 s of every boot. The kernel prints how long it took to get ready, so a slow addition shows: 136 ms on the Pi after this change.
- **`cargo pi` loads over JTAG, not a UART bootloader.** JTAG still works when the kernel has crashed or hung, and it needs no kernel code.

## How the console works

- `println!` writes to the UART, and to the screen once one is attached.
- Text uses a 16x30 bitmap font, packed to 1 bit per pixel by `scripts/make-font.py` from the old project's font sheet. The left 64 columns are text, all 36 rows down. The logo hovers over Aki's outstretched hand, turning about its vertical axis (`spin.rs`: one full turn at a steady speed in 3 s, starting and stopping at once, then 2 s facing front). It's in perspective, from 1,000 px away: squeezed flat, the picture went still whenever it faced the viewer squarely, so a turn looked like two flips with a pause between. Each column looks up its nearest source column and how near it is, and each pixel its nearest source pixel, using a cosine table built at compile time, as there's no floating point. `tools/spin_model.py` repeats that arithmetic so QEMU's screen can be compared exactly, with the square circling it, which saves and restores what it covers as the cursor does, since it crosses her hair. The square goes round 256 rainbow colors once a second, about every fourth one a frame, from `rainbow.rs`, which `scripts/make-rainbow.py` writes, as working them out needs floating point. They're at one perceived brightness, in OKLab: full-strength hues swing from 7% to 92% luminance, pure blue to yellow, and these stay within 39% to 45%. Each hue is as saturated as the screen can show at that brightness, smoothed, as the raw limit made bands. That makes blue and green softer than pure ones. Text stops short of the square's ring, which `square.rs` asserts. The cubes (`5`, `6`) take the logo's place while they spin, 168 px square, as wide as the logo facing front, so they stay inside its box, clear of Aki and the square, which carries on circling. The logo steps aside and comes back facing front. The GPU fills its whole drawing area, so that's sized to match, which `gpu_cube.rs` asserts.
- Text wraps back to the top instead of scrolling. Linux shows a better way to scroll than copying pixels: keep a grid of characters, and redraw only the cells that changed. See "How Linux's console scrolls so smoothly" in `hardware-notes.md`.
- **Serial carries packets alongside plain characters,** as `00 | COBS(type, payload, crc16) | 00`, in the `protocol` crate, which the kernel and server share. COBS makes a zero only ever a boundary, so damage costs one packet rather than everything after it, which was the weakness of the old project's framing. Only host-to-kernel packets exist so far. Commands are always packets, and plain characters are only ever echoed, as a lost boundary lets a packet's contents through as characters. Reliability is per message: a mouse move needs no acknowledgement, as the next one matters more. The page posts to `/mouse` and `/key`, and the server does the framing, so there's one implementation.
- **The cursor saves and restores what it covers,** as there are no layers. Each frame hides it, draws everything else, then shows it, so nothing it saved includes itself. It's a hand-drawn arrow, `kernel/src/cursor.png`, which `scripts/make-cursor.py` converts for the kernel to include, so it works with no images available. The tip of the arrow is the position, at (1, 0) in the sprite.
- **The kernel keeps running without a screen.** The firmware gave no framebuffer on a boot with the capture card unpowered, and a panic there would lock out anything done over serial. So it says so, and carries on with serial and the SD card, dropping drawing commands. It doesn't notice a display attached later.
- **The kernel updates files on the card by overwriting them in place,** through the `fat` crate, which has no code that can change the FAT or a directory. So those files keep their size, and `scripts/card.py` makes them a fixed size with padding. Established libraries like FatFs do much more than should be near a boot card. A failed write can leave that file bad, but not the filesystem or other files.
- **New kernels go through a trial boot.** `cargo upload trial` sends one to `kitsune-try.img`, and `cargo upload reboot --trial` has the kernel set the firmware's one-shot tryboot flag and reboot, which loads `tryboot.txt` once, and that names the trial file. Any later reset goes back to `kitsune-next.img`, so a bad kernel needs no card swap. Only then does `cargo upload kernel` replace the real one. Each upload stays in RAM until its CRC-32 passes, with every 240-byte chunk acknowledged and resent if need be.
- **Serial always comes first.** Drawing is far slower than the UART, whose small buffer overflows in about 1.4 ms, by our estimate. So input is echoed immediately and queued for the screen. Between characters, and between each scanline of clearing a new row, the kernel checks the UART again. The screen can lag during a burst, but serial bytes shouldn't be lost. More than 8 KB waiting means the extra doesn't reach the screen, but serial still gets it.
- Fault reports print four registers per row, so that the `FATAL EXCEPTION` line stays on screen.

## How images work

```
0x080000  the kernel    cargo pi replaces only this, and the linker enforces the limit
0x200000  the images    loaded from the SD card, and left alone by cargo pi
```

- **Baked into the card's image, but kept out of `cargo pi`.** Everything in the kernel would otherwise slow each run: at about 313 KiB/s, Aki alone would add about 5 s. `scripts/build-image` makes `target/kernel-only.img` for `cargo pi`, and `target/kitsune-next.img`, with the images, for the card. So after changing images, the card needs flashing again.
- **Fixed slots with spare room,** set in `scripts/make-assets.py`, so a slightly bigger version doesn't move the others. The region stays below `0x1000000`, which something overwrites during boot. A table at the start lets a kernel without images say so, instead of drawing garbage.
- **Format: run-length with a raw escape,** the idea known as PackBits. A byte of 1-254 is a run count, followed by one symbol. `0xFF` is followed by a length byte (0 means 256) and that many raw symbols. It was chosen as a first experiment. For Aki it's 1,486 KB, against 1,688 KB for plain run-length and 714 KB for QOI. The worst case costs 0.20% over raw.
- **A symbol is a full-color pixel or a palette index,** set per image in its header. Index 0 means skip the pixel. The same decoder handles all of them, so 1-byte indexes with a reduced Aki need no kernel change.
- **The logo uses 4 colors,** reduced on the host from 16 first. Going straight to 4 left rogue orange pixels around its white outline: in raw RGB math, a mid-gray like `#6d6d6d` is closer to orange than to black or white. Stepping through 16 happens to fix that for the logo, because its top 16 are its 4 main colors plus grays. Perceptual distance also fixed it, and it's the better bet for Aki, as it fixes the cause. A lossless palette for the logo would need 2-byte indexes for its 827 solid colors, at 16.5 KB, against 5.7 KB for 4 colors and 26.3 KB in full color.
- **Aki has a PC-98 look,** set in `scripts/pc98.py`: 16 hand-picked colors at 4 bits per channel, chosen by perceptual distance, with an 8x8 ordered dither. She's 427 KB, against 1,486 KB in full color. The automatic palette turned her open mouth orange and lost her blush, as pink is rare in the base image.
- **Areas limit colors where a palette can't.** Pink is only allowed on her face and hands, since it otherwise bleeds into her hair and clothes. Dark, cool pixels on her ribbon only use dark colors, which keeps light dots out, and pink on her held-out wrist is softened into a dithered blush. These are polygons in the base image's pixels.
- **Overlays can't leave a seam,** because the dither only depends on a pixel's color and its position in the base image. So rendering an overlay at its own position gives what dithering a true composite and cutting it out would. `scripts/test_pc98.py` checks that. Error diffusion would break this, as it spreads across edges. An overlay with its own palette and pattern showed an obvious pale rectangle.
- **Aki comes in four versions, and `0` clears the screen.** Key `1` is the original in full color, and `2`, `3` and `4` are the PC-98 look at x1, x2 and x3, with x1 shown at startup. `kernel/src/pictures.rs` holds them. Having the original pushed the image region to 6 MB. x2 and x3 are stored as ordinary full-size images, with each dot written out as 2x2 or 3x3 pixels, so the decoder needs no scaling, at the cost of storage: 384 KB and 339 KB, against about 114 KB and 52 KB stored small. Their mouths are cut from a true composite along whole blocks, which moves x3's to (366, 219). A version drawn over another leaves any of the old one that the new one skips, until a clear. Clearing removes the logo and text too, and nothing redraws them.
- **PNGs stay as the source art,** converted on the host.
- **Any pixel that isn't fully solid is skipped.** There's no blending yet, so the logo's soft edges look jagged.

## How the video reaches the browser

The capture card already makes JPEGs, and `ffmpeg` only copies them, so nothing is encoded here. The server writes each new frame once to every open `/video` connection and flushes it, and the page reads that one stream, decodes only the newest frame, and draws it straight to the screen. A viewer that falls behind skips frames. A frame over 2 seconds old is never sent, and the page calls the picture old and reconnects if nothing new arrives for that long. `/frame.jpg` stays for tools.

H.264 was considered and left out, as it's only worth its encoding delay and softer pixels when bandwidth is short, and this runs over localhost or a fast LAN.

These were measured on 2026-11-06 at 1920x1080, in a headless browser with no GPU on the same machine, with `tools/page_test/perf.mjs`:

| | Frames fetched | Data | Browser CPU | Server CPU |
| --- | --- | --- | --- | --- |
| One request per frame | 83/s, only 30 different | 20 MB/s | 135% of a core | 40% |
| Stream, with no second canvas | 30/s | 6.3 MB/s | 35 to 41% | 13% |
| The same, with a release server | 30/s | 6.3 MB/s | 35 to 41% | 1% |

**Flush each frame.** tiny_http's chunked responses held each frame's end until the next frame pushed it out, a frame late (33 ms), which a frame rate can't show. Measuring the silence between frames does: it was 0.0 ms, and is 30 ms now, with `Request::into_writer` and a flush per frame. `ffmpeg` wasn't holding anything back, with or without `-flush_packets`.

## How the sound reaches the browser

The Pi's headphone jack is wired to this PC's line-in, and the server records it with `parec` as raw 48 kHz stereo 16-bit samples, in 20 ms chunks. Each `/audio` connection gets every chunk from when it joined, in order, flushed one at a time: unlike video, a skipped chunk is a gap you can hear. The server keeps the last 32 chunks, 640 ms, and a listener further behind than that skips to the oldest kept. There's no encoding, at about 188 KiB/s per listener, for the same reason video isn't re-encoded.

The page loads its player, `server/src/sound.js`, the first time `s` or `7` is pressed, as browsers only allow sound after a key or click. `s` toggles sound, and `7` turns it on, waiting until it's listening before sending the key, since a listener only gets sound from when it joined, so the tone could otherwise start unheard. It schedules each piece to play straight after the one before, starting 80 ms ahead of now after a gap, and drops pieces while more than 400 ms is queued, so sound can't drift ever further behind the video.

Measured 2026-11-07 with `tools/page_test/sound_live.mjs` in headless Chromium on this machine: 147 pieces in about 3 s, queued 80 to 221 ms ahead, and the Pi's test tone arriving at 0.59 of full scale, against 0.003 for silence. That's about 0.1 to 0.25 s behind live, plus `parec`'s 20 ms. Firefox hasn't been tried.

## How to test on the real board

These were each learned the hard way:

- **Watch a test fail.** Break the code on purpose and check that the test notices. It has repeatedly found gaps in tests that passed, such as a `VecDeque` wraparound no test reached, and a default value compared against its own constant.
- **Test against the real tool early.** A fake OpenOCD happily returned `verified` from `verify_image`, which the real one never does on this chip.
- **Reset between hardware experiments,** with `tools/jtag/experiments/reset.py`. `cargo pi` leaves hardware state behind, and stacked tests gave misleading results.
- **Keep test builds away from risky addresses.** A mutation that wrote pixels into the peripheral range wedged the debug path, and only a power cycle could recover it.
- **Check what's on screen by reading it,** with `tools/screen_text.py` or by locating shapes in a captured frame. It found real bugs that a glance at a screenshot might have missed, such as one missing character after a wrap.
- **Don't flood a shared device without warning.** The user watches the same serial output and screen.
- **Compare images exactly in QEMU, and by shape on the Pi.** QEMU's screen dump gives exact pixels. The capture card changes colors too much for that, so `tools/check_images.py` checks only which pixels are lit.
- **A mutation that changes nothing means an untested path.** Breaking "0 means 256" changed no pixels, because no real image has a full raw packet. A generated noise image then tested it properly.
- **Launch scripted Chromium with `--password-store=basic`.** Otherwise it pops up KDE Wallet on the user's desktop, and navigation hangs.
- **Beware of sampling.** The capture is sampled well below the display's 60 Hz, so a clean result can hide brief problems, and flipping every frame can look like no flipping at all.
