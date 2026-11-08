//! The firmware's property mailbox, which is how the CPU asks the GPU for things.

use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use super::{PERIPHERAL_BASE, UNCACHED_VIEW};
use crate::video::Framebuffer;

const MAILBOX_BASE: usize = PERIPHERAL_BASE + 0xB880;
const MAILBOX_READ: *const u32 = MAILBOX_BASE as *const u32;
const MAILBOX_STATUS: *const u32 = (MAILBOX_BASE + 0x18) as *const u32;
const MAILBOX_WRITE: *mut u32 = (MAILBOX_BASE + 0x20) as *mut u32;

const STATUS_FULL: u32 = 1 << 31;
const STATUS_EMPTY: u32 = 1 << 30;
/// The low 4 bits of a mailbox value hold the channel, so messages must be
/// aligned to 16 bytes to leave those bits free.
const CHANNEL_MASK: u32 = 0xF;
const PROPERTY_CHANNEL: u32 = 8;

const REQUEST: u32 = 0;
const RESPONSE_OK: u32 = 0x8000_0000;
const END_TAG: u32 = 0;

const TAG_GET_MAX_CLOCK_RATE: u32 = 0x0003_0004;
const TAG_GET_TEMPERATURE: u32 = 0x0003_0006;
/// The only temperature sensor the firmware has.
const TEMPERATURE_ID: u32 = 0;
const TAG_SET_CLOCK_RATE: u32 = 0x0003_8002;
const TAG_NOTIFY_REBOOT: u32 = 0x0003_0048;
const TAG_SET_REBOOT_FLAGS: u32 = 0x0003_8064;
/// Boots `tryboot.txt` rather than `config.txt` once. The firmware clears it itself.
const REBOOT_FLAG_TRYBOOT: u32 = 1;
const CLOCK_ARM: u32 = 3;
/// Lets the firmware apply its normal turbo voltage for the rate.
const ALLOW_TURBO: u32 = 0;
const HERTZ_PER_MEGAHERTZ: u32 = 1_000_000;

const TAG_ALLOCATE_BUFFER: u32 = 0x0004_0001;
const TAG_GET_PITCH: u32 = 0x0004_0008;
const TAG_SET_PHYSICAL_SIZE: u32 = 0x0004_8003;
const TAG_SET_VIRTUAL_SIZE: u32 = 0x0004_8004;
const TAG_SET_DEPTH: u32 = 0x0004_8005;
const TAG_SET_PIXEL_ORDER: u32 = 0x0004_8006;

const BITS_PER_PIXEL: u32 = 32;
/// Blue first in memory, which makes a pixel's value read as `0xAARRGGBB`.
const PIXEL_ORDER_BGR: u32 = 0;
const BUFFER_ALIGNMENT: u32 = 4096;

/// The GPU sees RAM through a different address range. Clearing the top bits
/// gives the address the CPU uses.
const BUS_ADDRESS_MASK: u32 = 0x3FFF_FFFF;

const MESSAGE_WORDS: usize = 36;

/// Whole cache lines of their own, so nothing cached shares memory with the message.
#[repr(C, align(64))]
struct Message([u32; MESSAGE_WORDS]);

/// The GPU reads and writes this directly, so the CPU only uses it through the uncached
/// view, which needs the MMU to be on.
static mut MESSAGE: Message = Message([0; MESSAGE_WORDS]);

// Where the firmware writes each answer, given the message built below.
const WORD_RESPONSE: usize = 1;
const WORD_WIDTH: usize = 5;
const WORD_HEIGHT: usize = 6;
const WORD_DEPTH: usize = 15;
const WORD_ORDER: usize = 19;
const WORD_BUFFER: usize = 23;
const WORD_PITCH: usize = 28;
/// Where a clock tag's answer is: the rate, after the tag's header and the clock's id.
const WORD_CLOCK_RATE: usize = 6;

/// Sends these words and waits for the firmware's answer, which it writes back into
/// the same memory. Returns the answer, or `None` if the firmware reports failure.
fn call(words: &[u32]) -> Option<*const u32> {
    let physical = (&raw mut MESSAGE) as usize;
    let pointer = (physical + UNCACHED_VIEW) as *mut u32;
    let value = (physical as u32 & !CHANNEL_MASK) | PROPERTY_CHANNEL;
    for index in 0..MESSAGE_WORDS {
        // SAFETY: inside the message, and through a view the MMU maps.
        unsafe { write_volatile(pointer.add(index), words.get(index).copied().unwrap_or(0)) };
    }

    // SAFETY: these are valid mailbox registers on this board. The volatile
    // reads matter because the GPU changes the message, unknown to the compiler.
    // The barrier matters with the MMU on, as the message must be in RAM before the
    // mailbox tells the GPU about it.
    unsafe {
        asm!("dsb sy", options(nostack));
        while read_volatile(MAILBOX_STATUS) & STATUS_FULL != 0 {}
        write_volatile(MAILBOX_WRITE, value);

        loop {
            while read_volatile(MAILBOX_STATUS) & STATUS_EMPTY != 0 {}
            if read_volatile(MAILBOX_READ) == value {
                let good = read_volatile(pointer.add(WORD_RESPONSE)) == RESPONSE_OK;
                return good.then_some(pointer.cast_const());
            }
        }
    }
}

/// Sends one tag, and returns the word of the answer that holds a clock's rate.
fn ask_clock(tag: &[u32]) -> Option<u32> {
    let mut words = [0; MESSAGE_WORDS];
    words[0] = (MESSAGE_WORDS * 4) as u32;
    words[1] = REQUEST;
    words[2..2 + tag.len()].copy_from_slice(tag);
    let answer = call(&words)?;
    // SAFETY: inside the message. Volatile, as the GPU wrote this.
    Some(unsafe { read_volatile(answer.add(WORD_CLOCK_RATE)) })
}

/// Tells the firmware a reboot is coming, as Linux does, optionally as a one-off trial.
pub fn prepare_reboot(trial: bool) {
    if trial {
        // A failure shows up as the usual kernel booting, which the host notices.
        let _ = ask_clock(&[TAG_SET_REBOOT_FLAGS, 4, 0, REBOOT_FLAG_TRYBOOT, END_TAG]);
    }
    let _ = ask_clock(&[TAG_NOTIFY_REBOOT, 0, 0, END_TAG]);
}

/// The firmware's own reading of the chip's temperature, in thousandths of a degree.
/// Its answer has the same layout as a clock rate's: an ID, then the value.
pub fn firmware_temperature() -> Option<u32> {
    ask_clock(&[TAG_GET_TEMPERATURE, 8, 0, TEMPERATURE_ID, 0, END_TAG])
}

/// Raises the CPU to the firmware's own maximum, as it starts us at less than half
/// of it. Returns the rate in MHz, or `None` if the firmware doesn't offer one.
pub fn raise_cpu_clock() -> Option<u32> {
    let maximum = ask_clock(&[TAG_GET_MAX_CLOCK_RATE, 8, 0, CLOCK_ARM, 0, END_TAG])?;
    if maximum == 0 {
        return None;
    }
    let rate = ask_clock(&[TAG_SET_CLOCK_RATE, 12, 0, CLOCK_ARM, maximum, ALLOW_TURBO, END_TAG])?;
    Some(rate / HERTZ_PER_MEGAHERTZ)
}

/// Asks the firmware for a framebuffer. Returns `None` if it refuses, or gives us
/// something other than what we asked for.
pub fn framebuffer(width: u32, height: u32) -> Option<Framebuffer> {
    let words = [
        (MESSAGE_WORDS * 4) as u32,
        REQUEST,
        // Each tag is: tag, value size in bytes, request code, then its values.
        TAG_SET_PHYSICAL_SIZE, 8, 0, width, height,
        TAG_SET_VIRTUAL_SIZE, 8, 0, width, height,
        TAG_SET_DEPTH, 4, 0, BITS_PER_PIXEL,
        TAG_SET_PIXEL_ORDER, 4, 0, PIXEL_ORDER_BGR,
        TAG_ALLOCATE_BUFFER, 8, 0, BUFFER_ALIGNMENT, 0,
        TAG_GET_PITCH, 4, 0, 0,
        END_TAG,
    ];
    let message = call(&words)?;

    let answer = |index: usize| {
        // SAFETY: every index used below is inside the message.
        unsafe { read_volatile(message.add(index)) }
    };
    let as_requested = answer(WORD_WIDTH) == width
        && answer(WORD_HEIGHT) == height
        && answer(WORD_DEPTH) == BITS_PER_PIXEL
        && answer(WORD_ORDER) == PIXEL_ORDER_BGR;
    let base = answer(WORD_BUFFER) & BUS_ADDRESS_MASK;
    if !as_requested || base == 0 {
        return None;
    }

    Some(Framebuffer {
        base: base as usize + UNCACHED_VIEW,
        width: width as usize,
        height: height as usize,
        pitch: answer(WORD_PITCH) as usize,
    })
}
