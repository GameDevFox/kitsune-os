//! Throwaway kernel module: times full-screen fills, with the MMU off and on.
//! See run.py in this folder. Needs a reset afterward, as it leaves the MMU on.

use core::arch::asm;
use core::ptr::write_volatile;

use crate::board;
use crate::video::Framebuffer;

const RUNS: u32 = 3;
/// Unused RAM the same size as the screen, to compare with the framebuffer.
const PLAIN_RAM: usize = 0x1000_0000;

// A pixel value, dark so that the screen stays dim. Each test paints its own.
const DARK: [u64; 7] = [
    0xFF10_1010_FF10_1010,
    0xFF18_1010_FF18_1010,
    0xFF10_1810_FF10_1810,
    0xFF10_1018_FF10_1018,
    0xFF18_1810_FF18_1810,
    0xFF18_1018_FF18_1018,
    0xFF10_1818_FF10_1818,
];

// With the MMU on, the same physical RAM is reachable at these virtual offsets.
const NON_CACHEABLE_ALIAS: usize = 0x4000_0000;
const WRITE_BACK_ALIAS: usize = 0x8000_0000;

const BLOCK: u64 = 0b01;
const ACCESS_FLAG: u64 = 1 << 10;
const INNER_SHAREABLE: u64 = 0b11 << 8;
const EXECUTE_NEVER: u64 = (1 << 53) | (1 << 54);
const ATTR_WRITE_BACK: u64 = 0 << 2;
const ATTR_NON_CACHEABLE: u64 = 1 << 2;
const ATTR_DEVICE: u64 = 2 << 2;
/// Attribute 0 is write-back, 1 is non-cacheable, 2 is device.
const MAIR: u64 = 0x00_44_FF;
/// 39-bit addresses starting at level 1, 4 KB pages, walks cached, no TTBR1.
const TCR: u64 = 25 | (0b01 << 8) | (0b01 << 10) | (0b11 << 12) | (1 << 23);
const SCTLR_MMU: u64 = 1 << 0;
const SCTLR_DATA_CACHE: u64 = 1 << 2;
const SCTLR_INSTRUCTION_CACHE: u64 = 1 << 12;
const ONE_GB: u64 = 0x4000_0000;
const CACHE_LINE: usize = 64;

#[repr(C, align(4096))]
struct Table([u64; 512]);
static mut TABLE: Table = Table([0; 512]);

/// Identity-maps the first GB as cached and the top GB as device memory, and
/// adds the first GB again at two aliases with other memory types.
fn enable_mmu() {
    // SAFETY: nothing else uses the table, and every address the kernel uses
    // stays mapped to the same place when the MMU turns on.
    unsafe {
        let table = &raw mut TABLE;
        let normal = BLOCK | ACCESS_FLAG | INNER_SHAREABLE;
        (*table).0[0] = normal | ATTR_WRITE_BACK;
        (*table).0[1] = normal | ATTR_NON_CACHEABLE | EXECUTE_NEVER;
        (*table).0[2] = normal | ATTR_WRITE_BACK | EXECUTE_NEVER;
        (*table).0[3] = (3 * ONE_GB) | BLOCK | ACCESS_FLAG | ATTR_DEVICE | EXECUTE_NEVER;
        let base = table as usize as u64;
        let sctlr = SCTLR_MMU | SCTLR_DATA_CACHE | SCTLR_INSTRUCTION_CACHE;
        asm!(
            "msr mair_el1, {mair}",
            "msr tcr_el1, {tcr}",
            "msr ttbr0_el1, {base}",
            "isb",
            "tlbi vmalle1",
            "dsb sy",
            "isb",
            "mrs {value}, sctlr_el1",
            "orr {value}, {value}, {sctlr}",
            "msr sctlr_el1, {value}",
            "isb",
            mair = in(reg) MAIR,
            tcr = in(reg) TCR,
            base = in(reg) base,
            sctlr = in(reg) sctlr,
            value = out(reg) _,
        );
    }
}

fn fill_words(base: usize, bytes: usize, value: u64) {
    for offset in (0..bytes).step_by(4) {
        // SAFETY: inside the region each test passes.
        unsafe { write_volatile((base + offset) as *mut u32, value as u32) };
    }
}

/// Sixteen bytes per write, like a tuned clear routine.
fn fill_pairs(base: usize, bytes: usize, value: u64) {
    let mut pointer = base;
    let end = base + bytes;
    // SAFETY: inside the region each test passes, and `base` is 16-byte aligned.
    unsafe {
        asm!(
            "2:",
            "stp {value}, {value}, [{pointer}], #16",
            "cmp {pointer}, {end}",
            "b.lo 2b",
            pointer = inout(reg) pointer,
            value = in(reg) value,
            end = in(reg) end,
            options(nostack),
        );
    }
    let _ = pointer;
}

/// Zeroes 64 bytes at a time, without even reading the cache line first.
fn zero_lines(base: usize, bytes: usize) {
    let mut pointer = base;
    let end = base + bytes;
    while pointer < end {
        // SAFETY: inside the region each test passes; needs cacheable memory.
        unsafe { asm!("dc zva, {}", in(reg) pointer, options(nostack)) };
        pointer += CACHE_LINE;
    }
}

/// Pushes cached data out to RAM, where the display can see it.
fn clean_cache(base: usize, bytes: usize) {
    let mut pointer = base;
    let end = base + bytes;
    while pointer < end {
        // SAFETY: inside the region each test passes.
        unsafe { asm!("dc cvac, {}", in(reg) pointer, options(nostack)) };
        pointer += CACHE_LINE;
    }
    // SAFETY: only a barrier.
    unsafe { asm!("dsb sy", options(nostack)) };
}

fn time(name: &str, bytes: usize, mut work: impl FnMut()) {
    let mut best = u32::MAX;
    for _ in 0..RUNS {
        let start = board::microseconds();
        work();
        best = best.min(board::microseconds().wrapping_sub(start));
    }
    let megabytes = bytes as u64 / u64::from(best.max(1));
    let fps_tenths = 10_000_000 / u64::from(best.max(1));
    println!(
        "{name:44} {:>7} us {megabytes:>6} MB/s {:>5}.{} fps",
        best,
        fps_tenths / 10,
        fps_tenths % 10
    );
}

pub fn run(framebuffer: &Framebuffer) -> ! {
    let bytes = framebuffer.pitch * framebuffer.height;
    let screen = framebuffer.base;
    println!("--- bytes per frame: {bytes}");

    println!("--- MMU off: everything is device memory");
    time("framebuffer, 4 bytes per write", bytes, || fill_words(screen, bytes, DARK[0]));
    time("framebuffer, 16 bytes per write", bytes, || fill_pairs(screen, bytes, DARK[1]));
    time("plain RAM, 4 bytes per write", bytes, || fill_words(PLAIN_RAM, bytes, 0));
    time("plain RAM, 16 bytes per write", bytes, || fill_pairs(PLAIN_RAM, bytes, 0));

    enable_mmu();
    println!("--- MMU on");

    let uncached = screen + NON_CACHEABLE_ALIAS;
    time("framebuffer non-cacheable, 4 bytes", bytes, || fill_words(uncached, bytes, DARK[2]));
    time("framebuffer non-cacheable, 16 bytes", bytes, || fill_pairs(uncached, bytes, DARK[3]));

    let ram_cached = PLAIN_RAM + WRITE_BACK_ALIAS;
    time("plain RAM write-back, 16 bytes", bytes, || fill_pairs(ram_cached, bytes, 0));
    time("plain RAM write-back, zero 64-byte lines", bytes, || zero_lines(ram_cached, bytes));

    let cached = screen + WRITE_BACK_ALIAS;
    time("framebuffer write-back + clean, 16 bytes", bytes, || {
        fill_pairs(cached, bytes, DARK[4]);
        clean_cache(cached, bytes);
    });
    time("framebuffer write-back, zero lines + clean", bytes, || {
        zero_lines(cached, bytes);
        clean_cache(cached, bytes);
    });

    // Leave a known color, to confirm the cached path really reached the screen.
    fill_pairs(cached, bytes, DARK[5]);
    clean_cache(cached, bytes);
    println!("DONE");
    board::console_write(b"");
    loop {
        // SAFETY: only waits for an event.
        unsafe { asm!("wfe", options(nostack)) };
    }
}
