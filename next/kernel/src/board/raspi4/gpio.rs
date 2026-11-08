//! The BCM2711's GPIO pins: each pin's function, and setting its level.
//! Register layout from the datasheet's chapter 5.

use core::ptr::{read_volatile, write_volatile};

const GPFSEL0: usize = 0x00;
const GPSET0: usize = 0x1C;
const GPCLR0: usize = 0x28;
const PINS_PER_FUNCTION_WORD: u32 = 10;
const FUNCTION_BITS: u32 = 3;
const FUNCTION_MASK: u32 = 0b111;
const FUNCTION_OUTPUT: u32 = 0b001;
const PINS_PER_WORD: u32 = 32;
const WORD_BYTES: usize = 4;

#[derive(Clone, Copy)]
pub struct Gpio {
    base: usize,
}

impl Gpio {
    /// `base` must be the GPIO's registers, in device memory.
    pub const fn new(base: usize) -> Self {
        Self { base }
    }

    fn register(&self, first: usize, pin: u32, pins_per_word: u32) -> *mut u32 {
        (self.base + first + (pin / pins_per_word) as usize * WORD_BYTES) as *mut u32
    }

    /// Makes `pin` an output. Only its own bits change: a register holds ten pins'
    /// functions, such as 40 and 41's, which give the headphone jack to the PWM.
    pub fn make_output(&self, pin: u32) {
        let register = self.register(GPFSEL0, pin, PINS_PER_FUNCTION_WORD);
        let shift = pin % PINS_PER_FUNCTION_WORD * FUNCTION_BITS;
        // SAFETY: the caller of `new` vouched for these registers. Nothing else changes
        // this register while the kernel runs on one core.
        unsafe {
            let value = read_volatile(register);
            write_volatile(register, value & !(FUNCTION_MASK << shift) | FUNCTION_OUTPUT << shift);
        }
    }

    /// Drives an output pin high or low. Setting and clearing touch only this pin, so
    /// it's safe from an interrupt handler.
    pub fn set(&self, pin: u32, high: bool) {
        let register = self.register(if high { GPSET0 } else { GPCLR0 }, pin, PINS_PER_WORD);
        // SAFETY: as in `make_output`.
        unsafe { write_volatile(register, 1 << (pin % PINS_PER_WORD)) };
    }
}
