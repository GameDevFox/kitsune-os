//! The BCM2711's system timer: a 64-bit counter at 1 MHz from reset, and four compare
//! channels that raise an interrupt when the counter's low 32 bits reach them. Register
//! layout from the datasheet's chapter 10.

use core::ptr::{read_volatile, write_volatile};

const CS: usize = 0x00;
const CLO: usize = 0x04;
const CHI: usize = 0x08;
const COMPARE_0: usize = 0x0C;
const WORD_BYTES: usize = 4;
const HALF_BITS: u32 = 32;

/// A compare channel the ARM may use. The GPU's firmware uses 0 and 2, as far as we know
/// from other code; the datasheet doesn't say.
#[derive(Clone, Copy)]
pub struct Channel(usize);

pub const CHANNEL_1: Channel = Channel(1);
pub const CHANNEL_3: Channel = Channel(3);

impl Channel {
    pub fn number(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct SystemTimer {
    base: usize,
}

impl SystemTimer {
    /// `base` must be the system timer's registers, in device memory.
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

    /// Microseconds since reset. The halves are read separately, so the high half is read
    /// again to catch the low half wrapping in between.
    pub fn now(&self) -> u64 {
        loop {
            let high = self.read(CHI);
            let low = self.read(CLO);
            if self.read(CHI) == high {
                return u64::from(high) << HALF_BITS | u64::from(low);
            }
        }
    }

    /// Raises the channel's interrupt when the counter reaches `at`, in microseconds since
    /// reset, clearing any match from before. Only the low 32 bits are compared, so `at`
    /// must be less than about 71 minutes ahead.
    pub fn set_alarm(&self, channel: Channel, at: u64) {
        self.clear(channel);
        self.write(COMPARE_0 + channel.0 * WORD_BYTES, at as u32);
    }

    /// Whether the channel's alarm has gone off since it was last cleared.
    pub fn matched(&self, channel: Channel) -> bool {
        self.read(CS) & 1 << channel.0 != 0
    }

    /// Clears the channel's match, which also drops its interrupt.
    pub fn clear(&self, channel: Channel) {
        self.write(CS, 1 << channel.0);
    }
}
