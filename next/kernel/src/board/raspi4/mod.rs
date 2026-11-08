//! Raspberry Pi 4 (BCM2711). The only module that knows this board's addresses.

use core::ptr::{read_volatile, write_volatile};

pub mod audio;
pub mod gpio;
pub mod gpu;
pub mod rng;
pub mod system_timer;
pub mod thermal;
pub mod watchdog;
mod mailbox;
mod sd;

pub use mailbox::{firmware_temperature, framebuffer, raise_cpu_clock};
pub use sd::{read_blocks, write_block, BLOCK_BYTES};

/// Where the images sit, with the size set aside for them. `scripts/make-assets.py`
/// must agree. The firmware loads them with the kernel, and `cargo pi` leaves them
/// alone. They stay below 0x1000000, which something overwrites during boot.
const ASSETS_BASE: usize = 0x20_0000;
const ASSETS_SIZE: usize = 6 * 1024 * 1024;

/// The power manager, holding the watchdog, when there's no device tree. Reboots are
/// essential, as `cargo upload reboot` needs them, so this has a fallback.
pub const WATCHDOG: usize = PERIPHERAL_BASE + 0x10_0000;
/// Enough for what's already in the UART's buffer to go out.
const REBOOT_DRAIN_MICROSECONDS: u32 = 20_000;

/// Restarts the Pi through `watchdog`, as Linux does, booting the usual kernel, or as a
/// one-off trial, the one `tryboot.txt` names.
pub fn reboot(watchdog: watchdog::Watchdog, trial: bool) -> ! {
    mailbox::prepare_reboot(trial);
    let start = microseconds();
    while microseconds().wrapping_sub(start) < REBOOT_DRAIN_MICROSECONDS {}
    watchdog.reboot()
}

/// RAM nothing else uses, for a file arriving over serial. It's cached, as only the CPU
/// touches it. It sits just past the GPU's memory.
const UPLOAD_BASE: usize = 0x1100_0000;
const UPLOAD_SIZE: usize = 1024 * 1024;

/// That memory. There's one user, which doesn't keep this across calls.
pub fn upload_memory() -> &'static mut [u8] {
    // SAFETY: see above, and the kernel runs on one core with interrupts masked.
    unsafe { core::slice::from_raw_parts_mut(UPLOAD_BASE as *mut u8, UPLOAD_SIZE) }
}

/// RAM nothing else uses, for a sound clip that `tools/voice.py` loads over JTAG. It's
/// cached, as only the CPU touches it, and sits just past the upload memory.
const SOUND_BASE: usize = 0x1200_0000;
const SOUND_SIZE: usize = 4 * 1024 * 1024;

/// That memory, which may not hold a clip. The kernel only reads it.
pub fn sound_region() -> &'static [u8] {
    // SAFETY: see above. Nothing in the kernel writes it.
    unsafe { core::slice::from_raw_parts(SOUND_BASE as *const u8, SOUND_SIZE) }
}

/// The GIC-400's distributor and CPU interface, for when there's no device tree to say.
/// The tree on this Pi gives the same, from `/soc`'s `ranges`.
pub const GIC_DISTRIBUTOR: usize = 0xFF84_1000;
pub const GIC_CPU_INTERFACE: usize = 0xFF84_2000;

/// The system timer, and the GIC's number for channel 0's interrupt, for when there's no
/// device tree. Channel N's is that plus N.
pub const SYSTEM_TIMER: usize = 0xFE00_3000;
pub const SYSTEM_TIMER_FIRST_INTERRUPT: u32 = 96;

/// The end of the CPU's share of RAM, as the firmware set it up on this board: the GPU
/// has the rest of the first 1 GB. See "This board" in `docs/hardware-notes.md`.
pub const CPU_MEMORY_END: usize = 0x3B40_0000;

extern "C" {
    /// Where `boot.s` saves the firmware's device tree pointer, which `link.ld` places.
    static __device_tree_slot: u64;
}

/// The device tree pointer the firmware gave this boot, or an earlier one if `cargo pi`
/// started the kernel. Unchecked: RAM survives a reboot, so it could be anything.
pub fn saved_device_tree_pointer() -> usize {
    // SAFETY: the slot is RAM outside the kernel, which only `boot.s` writes.
    unsafe { read_volatile(core::ptr::addr_of!(__device_tree_slot)) as usize }
}

/// How the MMU should treat memory.
#[derive(Clone, Copy)]
pub enum MemoryType {
    Normal,
    /// The first block's memory again, uncached.
    UncachedFirstBlock,
    Device,
}

/// Where `UncachedFirstBlock` appears, for what the GPU also uses: it reads RAM directly,
/// so it can't see what's in the cache. Nothing may use this memory through both views.
const UNCACHED_VIEW: usize = 0x4000_0000;

/// Each block of memory from address 0, 1 GB each. The last holds the peripherals, and
/// some RAM that nothing uses yet. Anything after these is unmapped.
pub const MEMORY_MAP: [MemoryType; 4] = [MemoryType::Normal, MemoryType::UncachedFirstBlock, MemoryType::Normal, MemoryType::Device];

/// Peripheral base in the BCM2711's default "low peripheral" address mode.
const PERIPHERAL_BASE: usize = 0xFE00_0000;

/// The system timer's low 32 bits, which count microseconds since boot.
const TIMER_LOW: *const u32 = (PERIPHERAL_BASE + 0x3004) as *const u32;

// PL011 UART registers.
const UART_BASE: usize = PERIPHERAL_BASE + 0x20_1000;
const UART_DATA: *mut u32 = UART_BASE as *mut u32;
const UART_FLAGS: *const u32 = (UART_BASE + 0x18) as *const u32;
const UART_LINE_CONTROL: *mut u32 = (UART_BASE + 0x2C) as *mut u32;
const UART_CONTROL: *mut u32 = (UART_BASE + 0x30) as *mut u32;

const FLAGS_RX_EMPTY: u32 = 1 << 4;
const FLAGS_TX_FULL: u32 = 1 << 5;

const LINE_CONTROL_FIFO_ENABLE: u32 = 1 << 4;
const LINE_CONTROL_8_BITS: u32 = 0b11 << 5;

const CONTROL_ENABLE: u32 = 1 << 0;
const CONTROL_TX_ENABLE: u32 = 1 << 8;
const CONTROL_RX_ENABLE: u32 = 1 << 9;

/// Enables the UART. On hardware this relies on `enable_uart=1` in config.txt
/// for pin routing and baud rate, which has not been tested yet.
pub fn console_init() {
    // SAFETY: these are valid PL011 registers on this board, and boot runs on a
    // single core with nothing else using the UART.
    unsafe {
        write_volatile(UART_CONTROL, 0);
        write_volatile(UART_LINE_CONTROL, LINE_CONTROL_FIFO_ENABLE | LINE_CONTROL_8_BITS);
        write_volatile(UART_CONTROL, CONTROL_ENABLE | CONTROL_TX_ENABLE | CONTROL_RX_ENABLE);
    }
}

/// Returns the next received byte, if one has arrived.
pub fn console_try_read() -> Option<u8> {
    // SAFETY: as in `console_init`. The data register holds error flags above
    // the data bits, which the truncation drops.
    unsafe {
        if read_volatile(UART_FLAGS) & FLAGS_RX_EMPTY != 0 {
            return None;
        }
        Some(read_volatile(UART_DATA) as u8)
    }
}

/// The memory set aside for images, which may not hold any. Call this only once, as it
/// hands out the only way in.
pub fn assets_region() -> &'static mut [u8] {
    // SAFETY: this is RAM the kernel doesn't otherwise use.
    unsafe { core::slice::from_raw_parts_mut(ASSETS_BASE as *mut u8, ASSETS_SIZE) }
}

pub fn microseconds() -> u32 {
    // SAFETY: this is the timer's read-only count on this board.
    unsafe { read_volatile(TIMER_LOW) }
}

pub fn console_write(bytes: &[u8]) {
    for &byte in bytes {
        // SAFETY: as in `console_init`.
        unsafe {
            while read_volatile(UART_FLAGS) & FLAGS_TX_FULL != 0 {}
            write_volatile(UART_DATA, u32::from(byte));
        }
    }
}
