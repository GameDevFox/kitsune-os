//! The watchdog in the BCM2711's power manager: a countdown that resets the whole board
//! when it reaches zero, unless it's fed first. Register details from Linux's
//! `bcm2835_wdt.c`. Every write needs the password in its top byte, so a stray write
//! can't set it off. It counts on while JTAG has the core halted.

use core::ptr::{read_volatile, write_volatile};

// From the power manager's start.
const RSTC: usize = 0x1C;
const RSTS: usize = 0x20;
const WDOG: usize = 0x24;
const PASSWORD: u32 = 0x5A00_0000;

/// The countdown's bits in `WDOG`.
const TICKS_MASK: u32 = 0x000F_FFFF;
/// The countdown runs at 65536 ticks a second, so the longest is about 16 s.
pub const TICKS_PER_SECOND: u32 = 1 << 16;
pub const MOST_TICKS: u32 = TICKS_MASK;
/// The reset configuration's bits in `RSTC`, which say what happens at zero.
const CONFIG_CLEAR: u32 = 0xFFFF_FFCF;
const CONFIG_FULL_RESET: u32 = 0x20;
/// Written to `RSTC` to cancel the countdown.
const STOP: u32 = 0x102;
/// The firmware reads a partition to boot from these bits of `RSTS`. Zero is the usual.
const PARTITION_CLEAR: u32 = 0xFFFF_FAAA;
/// How soon `reboot` resets the board: about 150 us.
const REBOOT_TICKS: u32 = 10;

#[derive(Clone, Copy)]
pub struct Watchdog {
    base: usize,
}

impl Watchdog {
    /// `base` must be the power manager's registers, in device memory.
    pub const fn new(base: usize) -> Self {
        Self { base }
    }

    fn read(&self, offset: usize) -> u32 {
        // SAFETY: the caller of `new` vouched for these registers.
        unsafe { read_volatile((self.base + offset) as *const u32) }
    }

    fn write(&self, offset: usize, value: u32) {
        // SAFETY: as in `read`. The password is added here, so no caller can forget it.
        unsafe { write_volatile((self.base + offset) as *mut u32, PASSWORD | value) };
    }

    /// Starts, or restarts, the countdown: the board resets in `ticks` ticks unless this
    /// is called again first, or `cancel` is. Longer than `MOST_TICKS` gets the most.
    pub fn arm(&self, ticks: u32) {
        self.write(WDOG, ticks.min(MOST_TICKS));
        self.write(RSTC, (self.read(RSTC) & CONFIG_CLEAR) | CONFIG_FULL_RESET);
    }

    /// Stops the countdown, so nothing resets.
    pub fn cancel(&self) {
        self.write(RSTC, STOP);
    }

    pub fn armed(&self) -> bool {
        self.read(RSTC) & CONFIG_FULL_RESET != 0
    }

    /// Ticks left before the reset.
    pub fn remaining(&self) -> u32 {
        self.read(WDOG) & TICKS_MASK
    }

    /// Resets the board at once, booting the usual partition.
    pub fn reboot(&self) -> ! {
        self.write(RSTS, self.read(RSTS) & PARTITION_CLEAR);
        self.arm(REBOOT_TICKS);
        loop {
            core::hint::spin_loop();
        }
    }
}
