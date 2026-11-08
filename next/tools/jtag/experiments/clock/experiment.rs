//! Throwaway kernel module: reads the CPU's clock from the firmware, raises it to the
//! firmware's own maximum, and repeats the tests in `stores.rs`. See run.py in this
//! folder. Needs a reset afterward, as it leaves the MMU and FPU on.

use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use crate::stores::{self, Memory, Target};
use crate::video::Framebuffer;

const MAILBOX_BASE: usize = 0xFE00_B880;
const MAILBOX_READ: *const u32 = MAILBOX_BASE as *const u32;
const MAILBOX_STATUS: *const u32 = (MAILBOX_BASE + 0x18) as *const u32;
const MAILBOX_WRITE: *mut u32 = (MAILBOX_BASE + 0x20) as *mut u32;
const STATUS_FULL: u32 = 1 << 31;
const STATUS_EMPTY: u32 = 1 << 30;
const PROPERTY_CHANNEL: u32 = 8;
const RESPONSE_OK: u32 = 0x8000_0000;
const MAILBOX_TIMEOUT_MICROSECONDS: u32 = 1_000_000;

/// Unused RAM for the message. It's reached by its plain address with the MMU
/// off, and through the non-cacheable mapping with it on, so the GPU sees it.
const MESSAGE: usize = 0x1F00_0000;
/// Which view of that the kernel's memory map offers, now that the MMU is always on.
pub const VIEW: usize = 0x4000_0000;
const MESSAGE_BYTES: u32 = 40;
const MESSAGE_WORDS: usize = 10;
const WORD_RESPONSE: usize = 1;
const WORD_VALUE: usize = 6;
const WORD_ONLY_VALUE: usize = 5;

pub const TAG_GET_RATE: u32 = 0x0003_0002;
pub const TAG_GET_MAX_RATE: u32 = 0x0003_0004;
const TAG_GET_MIN_RATE: u32 = 0x0003_0007;
pub const TAG_GET_MEASURED_RATE: u32 = 0x0003_0047;
pub const TAG_GET_TEMPERATURE: u32 = 0x0003_0006;
const TAG_GET_THROTTLED: u32 = 0x0003_0046;
const TAG_SET_RATE: u32 = 0x0003_8002;
pub const CLOCK_ARM: u32 = 3;
pub const CLOCK_CORE: u32 = 4;
const CLOCK_SDRAM: u32 = 8;
pub const TEMPERATURE_SENSOR: u32 = 0;
/// Let the firmware apply its normal turbo voltage for the rate.
const ALLOW_TURBO: u32 = 0;
const MEGAHERTZ: u32 = 1_000_000;
/// Time for the clock to settle before measuring.
const SETTLE_MICROSECONDS: u32 = 100_000;

const SPIN_LOOPS: u64 = 100_000_000;
const SMALL: usize = 16 * 1024;
const PASSES: usize = 512;

fn call(view: usize, words: &[u32]) -> Option<[u32; MESSAGE_WORDS]> {
    let pointer = (MESSAGE + view) as *mut u32;
    // SAFETY: unused RAM, and this board's mailbox registers. The volatile accesses
    // matter because the GPU changes the message, unknown to the compiler.
    unsafe {
        for index in 0..MESSAGE_WORDS {
            write_volatile(pointer.add(index), words.get(index).copied().unwrap_or(0));
        }
        asm!("dsb sy", options(nostack));
        let value = MESSAGE as u32 | PROPERTY_CHANNEL;
        while read_volatile(MAILBOX_STATUS) & STATUS_FULL != 0 {}
        write_volatile(MAILBOX_WRITE, value);

        let start = crate::board::microseconds();
        loop {
            if read_volatile(MAILBOX_STATUS) & STATUS_EMPTY == 0 && read_volatile(MAILBOX_READ) == value {
                break;
            }
            if crate::board::microseconds().wrapping_sub(start) > MAILBOX_TIMEOUT_MICROSECONDS {
                println!("mailbox timed out");
                return None;
            }
        }
        let mut answer = [0; MESSAGE_WORDS];
        for (index, word) in answer.iter_mut().enumerate() {
            *word = read_volatile(pointer.add(index));
        }
        (answer[WORD_RESPONSE] == RESPONSE_OK).then_some(answer)
    }
}

pub fn get(view: usize, tag: u32, id: u32) -> u32 {
    let message = [MESSAGE_BYTES, 0, tag, 8, 0, id, 0, 0];
    call(view, &message).map_or(0, |answer| answer[WORD_VALUE])
}

pub fn set_arm_rate(view: usize, rate: u32) -> u32 {
    let message = [MESSAGE_BYTES, 0, TAG_SET_RATE, 12, 0, CLOCK_ARM, rate, ALLOW_TURBO, 0];
    let granted = call(view, &message).map_or(0, |answer| answer[WORD_VALUE]);
    let start = crate::board::microseconds();
    while crate::board::microseconds().wrapping_sub(start) < SETTLE_MICROSECONDS {}
    granted
}

fn state(view: usize, name: &str, frequency: u64) {
    let measured = get(view, TAG_GET_MEASURED_RATE, CLOCK_ARM) / MEGAHERTZ;
    let told = get(view, TAG_GET_RATE, CLOCK_ARM) / MEGAHERTZ;
    let core = get(view, TAG_GET_MEASURED_RATE, CLOCK_CORE) / MEGAHERTZ;
    let sdram = get(view, TAG_GET_MEASURED_RATE, CLOCK_SDRAM) / MEGAHERTZ;
    let degrees = get(view, TAG_GET_TEMPERATURE, TEMPERATURE_SENSOR) / 1000;
    let message = [MESSAGE_BYTES, 0, TAG_GET_THROTTLED, 4, 0, 0, 0];
    let throttled = call(view, &message).map_or(0, |answer| answer[WORD_ONLY_VALUE]);

    let begin = stores::ticks();
    let mut count = SPIN_LOOPS;
    // SAFETY: only counts down.
    unsafe { asm!(".p2align 6", "2:", "subs {0}, {0}, #1", "b.ne 2b", inout(reg) count, options(nostack)) };
    let _ = count;
    let spin = u64::from(MEGAHERTZ) * SPIN_LOOPS / (stores::ticks() - begin).max(1) * frequency / u64::from(MEGAHERTZ) / u64::from(MEGAHERTZ);
    println!("--- {name}: ARM {measured} MHz (set {told}), spin {spin} M/s");
    println!("core {core}, SDRAM {sdram} MHz, {degrees} C, throttled {throttled:#x}");
}

fn device_tests(screen: usize, bytes: usize, frequency: u64) {
    let target = Target { label: "dev", memory: Memory::Device, base: screen, bytes, frequency };
    let lines = (bytes - stores::SPARE_BYTES) / stores::CACHE_LINE;
    let mut runs = stores::time(&target, || stores::pairs_64(screen, lines));
    stores::report("STP X 64", 0, &target, lines * stores::CACHE_LINE, &mut runs);
    let count = (bytes - stores::SPARE_BYTES) / 52;
    let mut runs = stores::time(&target, || stores::registers_13(screen, count));
    stores::report("13 regs 52", 0, &target, count * 52, &mut runs);
    let count = bytes / 4;
    let mut runs = stores::time(&target, || stores::words(screen, count));
    stores::report("STR W 4", 0, &target, count * 4, &mut runs);
}

fn memory_tests(non_cacheable: &Target, write_back: &Target) {
    let bytes = write_back.bytes;
    let lines = (bytes - stores::SPARE_BYTES) / stores::CACHE_LINE;
    for target in [non_cacheable, write_back] {
        let mut runs = stores::time(target, || stores::pairs_64(target.base, lines));
        stores::report("STP X 64", 0, target, lines * stores::CACHE_LINE, &mut runs);
        let count = (bytes - stores::SPARE_BYTES) / 52;
        let mut runs = stores::time(target, || stores::registers_13(target.base, count));
        stores::report("13 regs 52", 0, target, count * 52, &mut runs);
        let count = bytes / 4;
        let mut runs = stores::time(target, || stores::words(target.base, count));
        stores::report("STR W 4", 0, target, count * 4, &mut runs);

        // The same 16 KB again and again: the cache's own speed, where it applies.
        let small = Target { label: target.label, memory: Memory::NonCacheable, base: target.base, bytes: SMALL, frequency: target.frequency };
        stores::read_pairs(target.base, SMALL / stores::PAIR_BYTES);
        let mut runs = stores::time(&small, || {
            for _ in 0..PASSES {
                stores::read_pairs(target.base, SMALL / stores::PAIR_BYTES);
            }
        });
        stores::report("read 16K", 0, &small, SMALL * PASSES, &mut runs);
        let small_lines = SMALL / stores::CACHE_LINE;
        let mut runs = stores::time(&small, || {
            for _ in 0..PASSES {
                stores::pairs_64(target.base, small_lines);
            }
        });
        stores::report("fill 16K", 0, &small, SMALL * PASSES, &mut runs);
        stores::sparse_words(target.base, small_lines);
        let mut runs = stores::time(&small, || {
            for _ in 0..PASSES {
                stores::sparse_words(target.base, small_lines);
            }
        });
        stores::report("sparse", 0, &small, small_lines * 4 * PASSES, &mut runs);
    }
    stores::run_clean(write_back, "clean STP", || stores::pairs_64(write_back.base, lines));
}

pub fn run(framebuffer: &Framebuffer) -> ! {
    let bytes = framebuffer.pitch * framebuffer.height;
    let screen = framebuffer.base;
    let frequency = stores::measure_frequency();
    stores::enable_fpu();

    let low = get(0, TAG_GET_RATE, CLOCK_ARM);
    let minimum = get(0, TAG_GET_MIN_RATE, CLOCK_ARM);
    let maximum = get(0, TAG_GET_MAX_RATE, CLOCK_ARM);
    println!("=== ARM {} MHz, min {}, max {}", low / MEGAHERTZ, minimum / MEGAHERTZ, maximum / MEGAHERTZ);
    if low == 0 || maximum == 0 {
        println!("the firmware didn't answer\nDONE");
    } else {
        state(0, "MMU off, low", frequency);
        device_tests(screen, bytes, frequency);
        let granted = set_arm_rate(0, maximum);
        println!("asked for {} MHz, got {}", maximum / MEGAHERTZ, granted / MEGAHERTZ);
        state(0, "MMU off, high", frequency);
        device_tests(screen, bytes, frequency);
        set_arm_rate(0, low);

        stores::enable_mmu();
        let view = stores::NON_CACHEABLE_ALIAS;
        let non_cacheable = Target { label: "nc", memory: Memory::NonCacheable, base: screen + view, bytes, frequency };
        let write_back = Target { label: "wb", memory: Memory::WriteBack, base: screen + stores::WRITE_BACK_ALIAS, bytes, frequency };
        state(view, "MMU on, low", frequency);
        memory_tests(&non_cacheable, &write_back);
        set_arm_rate(view, maximum);
        state(view, "MMU on, high", frequency);
        memory_tests(&non_cacheable, &write_back);
        set_arm_rate(view, low);
        state(view, "MMU on, low again", frequency);
        memory_tests(&non_cacheable, &write_back);
        stores::clean(write_back.base, bytes, true);
        println!("DONE");
    }
    loop {
        // SAFETY: only waits for an event.
        unsafe { asm!("wfe", options(nostack)) };
    }
}
