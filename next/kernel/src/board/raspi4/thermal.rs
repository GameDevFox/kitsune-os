//! The BCM2711's temperature sensor, in its AVS monitor. Register details from Linux's
//! `bcm2711_thermal.c`; the conversion's numbers come from the device tree.

use core::ptr::read_volatile;

/// The sensor's status register, from the AVS monitor's start.
pub const STATUS_OFFSET: usize = 0x200;
/// Both set when the reading is valid.
const VALID: u32 = 1 << 16 | 1 << 10;
const DATA_MASK: u32 = 0x3FF;

/// The sensor's latest raw reading, or `None` if it isn't valid. `status` must be its
/// status register, in device memory.
pub fn read_raw(status: usize) -> Option<u32> {
    // SAFETY: the caller vouched for the register.
    let value = unsafe { read_volatile(status as *const u32) };
    (value & VALID != 0).then_some(value & DATA_MASK)
}
