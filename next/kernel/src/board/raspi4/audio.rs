//! Sound from the headphone jack, through PWM1 on GPIO 40 and 41, which the firmware
//! already gives to it. The CPU writes a whole sound into a buffer, and a DMA channel
//! copies it into the PWM's FIFO as the PWM asks for more, so the CPU is free meanwhile.
//! See "Audio from the headphone jack" in `docs/hardware-notes.md`.

use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use super::{microseconds, PERIPHERAL_BASE, UNCACHED_VIEW};

// The clock manager's PWM clock. Its registers aren't in the BCM2711 datasheet; these
// come from other code, and every write needs the password.
const CM_PWMCTL: *mut u32 = (PERIPHERAL_BASE + 0x10_10A0) as *mut u32;
const CM_PWMDIV: *mut u32 = (PERIPHERAL_BASE + 0x10_10A4) as *mut u32;
const CM_PASSWORD: u32 = 0x5A00_0000;
const CM_SOURCE_OSCILLATOR: u32 = 1;
const CM_ENABLE: u32 = 1 << 4;
const CM_BUSY: u32 = 1 << 7;
const CM_DIVISOR_SHIFT: u32 = 12;
const OSCILLATOR_HZ: u32 = 54_000_000;

/// Which function each of GPIO 40 to 49 has, three bits per pin.
const GPFSEL4: *const u32 = (PERIPHERAL_BASE + 0x20_0010) as *const u32;
/// GPIO 40's and 41's bits, and alternate function 0, which connects them to PWM1.
const GPIO_40_41_MASK: u32 = 0o77;
const GPIO_40_41_PWM: u32 = 0o44;
/// A small divisor, so that each sample has many clock ticks: more ticks, more levels.
const CLOCK_DIVISOR: u32 = 2;
const PWM_CLOCK_HZ: u32 = OSCILLATOR_HZ / CLOCK_DIVISOR;

const PWM1_BASE: usize = PERIPHERAL_BASE + 0x20_C800;
const CTL: *mut u32 = PWM1_BASE as *mut u32;
const STA: *mut u32 = (PWM1_BASE + 0x04) as *mut u32;
const RNG1: *mut u32 = (PWM1_BASE + 0x10) as *mut u32;
const RNG2: *mut u32 = (PWM1_BASE + 0x20) as *mut u32;

// Each channel enabled, reading the FIFO, in the PWM's own mode, which spreads each
// sample's pulses across its period rather than one high-then-low pulse.
const CTL_PWEN1: u32 = 1 << 0;
const CTL_USEF1: u32 = 1 << 5;
const CTL_CLRF: u32 = 1 << 6;
const CTL_PWEN2: u32 = 1 << 8;
const CTL_USEF2: u32 = 1 << 13;
const CTL_PLAY: u32 = CTL_PWEN1 | CTL_USEF1 | CTL_PWEN2 | CTL_USEF2;

const STA_EMPTY: u32 = 1 << 1;
/// The error flags, cleared by writing them back: write and read errors, gaps on each
/// channel, and a bus error.
const STA_ERRORS: u32 = 0b1_0011_1100;

/// About as close to 48 kHz as the clock divides: 27 MHz / 562 is 48,042.7 Hz, which
/// `SAMPLE_RATE` rounds down to 48,042. `tools/voice.py` must agree.
const RANGE: u32 = 562;
pub const SAMPLE_RATE: u32 = PWM_CLOCK_HZ / RANGE;
/// The largest sample, and silence is half of it.
pub const MAX_SAMPLE: u32 = RANGE;

/// How long the PWM needs after its clock starts, and between steps in setting it up.
/// Written back to back, the enable didn't take: the channels stayed idle with the
/// control register reading as enabled, until it was written again. rpi4-osdev waits
/// 2 ms after each step, and so does this.
const SETTLE_MICROSECONDS: u32 = 2000;

fn settle() {
    let start = microseconds();
    while microseconds().wrapping_sub(start) < SETTLE_MICROSECONDS {}
}

/// Whether the firmware connected the jack's pins to the PWM, which the kernel relies
/// on rather than doing itself. QEMU leaves them as plain inputs, and has no PWM at all:
/// writing to it there is a data abort.
pub fn available() -> bool {
    // SAFETY: this is the GPIO's function register, which is only read.
    unsafe { read_volatile(GPFSEL4) & GPIO_40_41_MASK == GPIO_40_41_PWM }
}

/// Starts the clock and the PWM, with the FIFO empty.
fn start() {
    // SAFETY: these are this board's clock manager and PWM registers, and nothing else
    // in the kernel uses them.
    unsafe {
        write_volatile(CTL, 0);
        // The divisor may only change while the clock is stopped and no longer busy.
        write_volatile(CM_PWMCTL, CM_PASSWORD | CM_SOURCE_OSCILLATOR);
        while read_volatile(CM_PWMCTL) & CM_BUSY != 0 {}
        write_volatile(CM_PWMDIV, CM_PASSWORD | CLOCK_DIVISOR << CM_DIVISOR_SHIFT);
        write_volatile(CM_PWMCTL, CM_PASSWORD | CM_SOURCE_OSCILLATOR | CM_ENABLE);
        // The PWM's registers can't take writes until its clock runs: before then, they
        // set its bus error flag.
        while read_volatile(CM_PWMCTL) & CM_BUSY == 0 {}
        settle();

        write_volatile(RNG1, RANGE);
        write_volatile(RNG2, RANGE);
        write_volatile(CTL, CTL_CLRF);
        settle();
        write_volatile(CTL, CTL_PLAY);
        settle();
        write_volatile(STA, STA_ERRORS);
    }
}

// Where the DMA reads RAM, and finds the PWM: the datasheet's "legacy master" addresses.
// The first 1 GB of RAM appears at 0xC0000000, and peripherals at 0x7E000000.
const BUS_RAM: u32 = 0xC000_0000;
/// PWM1's FIFO register, at offset 0x18.
const BUS_PWM1_FIFO: u32 = 0x7E20_C818;

/// RAM nothing else uses, for the samples and the control block that describes them.
/// The DMA reads RAM directly, so the CPU writes it through the uncached view. It sits
/// just past the sound region.
const DMA_MEMORY: usize = 0x1300_0000;
const DMA_MEMORY_BYTES: usize = 16 * 1024 * 1024;
/// A control block is 8 words, and must start on a 32-byte boundary.
const CONTROL_BLOCK_BYTES: usize = 32;
const SAMPLES_AT: usize = DMA_MEMORY + CONTROL_BLOCK_BYTES;
/// Each frame is two words: right, then left, as the PWM's channels take turns.
const FRAME_BYTES: usize = 8;
const WORD_BYTES: usize = 4;
/// The most frames a sound can have: about 43 seconds.
pub const MAX_FRAMES: usize = (DMA_MEMORY_BYTES - CONTROL_BLOCK_BYTES) / FRAME_BYTES;

/// The PWM's DMA control: enable, and the FIFO levels at which it asks for more, and
/// urgently. 7 is the reset value of both.
const DMAC: *mut u32 = (PWM1_BASE + 0x08) as *mut u32;
const DMAC_ENABLE: u32 = 1 << 31;
const DMAC_PANIC_SHIFT: u32 = 8;
const DMAC_THRESHOLD: u32 = 7;

/// DMA channel 5. Linux's device tree leaves channels 0, 2 and 4 to 10 to the ARM, so
/// the firmware shouldn't touch it, and 0 to 6 are full channels, with long transfers.
const DMA_CHANNEL: u32 = 5;
const DMA_BASE: usize = PERIPHERAL_BASE + 0x7000 + 0x100 * DMA_CHANNEL as usize;
const DMA_CS: *mut u32 = DMA_BASE as *mut u32;
const DMA_CONBLK_AD: *mut u32 = (DMA_BASE + 0x04) as *mut u32;
const DMA_DEBUG: *mut u32 = (DMA_BASE + 0x20) as *mut u32;
/// Each channel's enable bit, all on after the firmware, but that's not promised.
const DMA_ENABLE: *mut u32 = (PERIPHERAL_BASE + 0x7FF0) as *mut u32;

const CS_ACTIVE: u32 = 1 << 0;
const CS_END: u32 = 1 << 1;
const CS_ERROR: u32 = 1 << 8;
const CS_RESET: u32 = 1 << 31;

// Transfer information: increment through the samples, and pace writes by the PWM's
// DREQ. DREQ 1 is shared with DSI0, chosen by a mux the datasheet doesn't document; the
// firmware leaves it on the PWM, as rpi4-osdev's sound relies on too.
const TI_DEST_DREQ: u32 = 1 << 6;
const TI_SRC_INC: u32 = 1 << 8;
const TI_PERMAP_SHIFT: u32 = 16;
const PWM1_DREQ: u32 = 1;

/// Writes a sound into the DMA's buffer, before `play` starts it.
pub struct Samples {
    frames: usize,
}

impl Samples {
    pub fn new() -> Self {
        Self { frames: 0 }
    }

    /// Adds one sample per side, each up to `MAX_SAMPLE`. Returns false once full.
    pub fn push(&mut self, left: u32, right: u32) -> bool {
        if self.frames >= MAX_FRAMES {
            return false;
        }
        let at = SAMPLES_AT + self.frames * FRAME_BYTES + UNCACHED_VIEW;
        // SAFETY: inside the memory set aside above, through the uncached view. Channel 1,
        // on GPIO 40, is the right side: recorded on 2026-11-07.
        unsafe {
            write_volatile(at as *mut u32, right);
            write_volatile((at + WORD_BYTES) as *mut u32, left);
        }
        self.frames += 1;
        true
    }
}

fn write_control_block(words: [u32; CONTROL_BLOCK_BYTES / WORD_BYTES]) {
    for (index, word) in words.into_iter().enumerate() {
        // SAFETY: the control block's place in the memory set aside, uncached.
        unsafe { write_volatile((DMA_MEMORY + UNCACHED_VIEW + index * WORD_BYTES) as *mut u32, word) };
    }
}

/// Plays the samples in the background, cutting off anything already playing. Check
/// `playing`, then call `stop` once it's false.
pub fn play(samples: Samples) {
    stop();
    start();
    write_control_block([
        TI_DEST_DREQ | TI_SRC_INC | PWM1_DREQ << TI_PERMAP_SHIFT,
        BUS_RAM + SAMPLES_AT as u32,
        BUS_PWM1_FIFO,
        (samples.frames * FRAME_BYTES) as u32,
        0,
        0,
        0,
        0,
    ]);
    // SAFETY: as in `start`, and these are DMA channel 5's registers, which nothing else
    // here uses. The barrier makes sure the samples and control block are in RAM first.
    unsafe {
        asm!("dsb sy", options(nostack));
        write_volatile(DMAC, DMAC_ENABLE | DMAC_THRESHOLD << DMAC_PANIC_SHIFT | DMAC_THRESHOLD);
        write_volatile(DMA_ENABLE, read_volatile(DMA_ENABLE) | 1 << DMA_CHANNEL);
        write_volatile(DMA_CONBLK_AD, BUS_RAM + DMA_MEMORY as u32);
        write_volatile(DMA_CS, CS_END | CS_ACTIVE);
    }
}

/// Whether the sound is still playing: the DMA is still copying, or the PWM still has
/// samples left.
pub fn playing() -> bool {
    // SAFETY: as in `play`.
    unsafe { read_volatile(DMA_CS) & CS_ACTIVE != 0 || read_volatile(STA) & STA_EMPTY == 0 }
}

/// The PWM's read, write and bus error flags. Its gap flags aren't included, as every
/// sound ends with one, once the FIFO runs dry after the last sample.
const STA_PROBLEMS: u32 = 0b1_0000_1100;

/// What went wrong while playing, if anything: the DMA's status and debug registers, and
/// the PWM's status, if either reported an error.
pub fn problems() -> Option<(u32, u32, u32)> {
    // SAFETY: as in `play`.
    let (cs, debug, status) = unsafe { (read_volatile(DMA_CS), read_volatile(DMA_DEBUG), read_volatile(STA)) };
    (cs & CS_ERROR != 0 || status & STA_PROBLEMS != 0).then_some((cs, debug, status))
}

/// Stops the DMA, then turns the PWM and its clock off.
pub fn stop() {
    // SAFETY: as in `play`.
    unsafe {
        write_volatile(DMA_CS, CS_RESET);
        write_volatile(DMAC, 0);
        write_volatile(CTL, 0);
        write_volatile(CM_PWMCTL, CM_PASSWORD | CM_SOURCE_OSCILLATOR);
    }
}
