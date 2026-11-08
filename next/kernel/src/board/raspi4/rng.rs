//! The BCM2711's hardware random number generator, an RNG200. Register details from
//! Linux's `iproc-rng200.c`, which handles this chip's version as `bcm2711_rng200`.

use core::ptr::{read_volatile, write_volatile};

const CTRL: usize = 0x00;
const TOTAL_BIT_COUNT: usize = 0x0C;
const TOTAL_BIT_COUNT_THRESHOLD: usize = 0x10;
const FIFO_DATA: usize = 0x20;
const FIFO_COUNT: usize = 0x24;

/// The generator's enable bits. Any set means it's already running, as the firmware or
/// an earlier kernel may have left it.
const CTRL_ENABLE_MASK: u32 = 0x1FFF;
/// Linux's settings: a divider for about 1 MHz sampling, and every enable bit.
const CTRL_DIVIDER: u32 = 0x3 << 13;
/// Its first bits are less random, so this many are thrown away.
const WARM_UP_BITS: u32 = 0x40000;
/// Linux waits for the bit count to pass this before reading.
const WARMED_UP: u32 = 16;
const FIFO_COUNT_MASK: u32 = 0xFF;
/// The FIFO level for its "full" interrupt, which nothing here uses, set as Linux does.
const FIFO_THRESHOLD: u32 = 2 << 8;

#[derive(Clone, Copy)]
pub struct Rng {
    base: usize,
}

impl Rng {
    /// `base` must be the generator's registers, in device memory.
    pub const fn new(base: usize) -> Self {
        Self { base }
    }

    fn read(&self, offset: usize) -> u32 {
        // SAFETY: the caller of `new` vouched for these registers.
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    fn write(&self, offset: usize, value: u32) {
        // SAFETY: as in `read`.
        unsafe { write_volatile((self.base + offset) as *mut u32, value) };
    }

    /// Starts the generator, unless it's already running.
    pub fn start(&self) {
        if self.read(CTRL) & CTRL_ENABLE_MASK != 0 {
            return;
        }
        self.write(TOTAL_BIT_COUNT_THRESHOLD, WARM_UP_BITS);
        self.write(FIFO_COUNT, FIFO_THRESHOLD);
        self.write(CTRL, CTRL_DIVIDER | CTRL_ENABLE_MASK);
    }

    /// The next random word, once there is one, or `None` if none is ready.
    pub fn try_next(&self) -> Option<u32> {
        let ready = self.read(TOTAL_BIT_COUNT) > WARMED_UP && self.read(FIFO_COUNT) & FIFO_COUNT_MASK > 0;
        ready.then(|| self.read(FIFO_DATA))
    }
}
