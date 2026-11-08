//! Throwaway kernel module: how store width, chunk size, alignment and memory type
//! affect a full-screen fill. See run.py in this folder. Needs a reset afterward, as
//! it leaves the MMU and the FPU on, which the normal kernel doesn't expect.

use core::arch::asm;

use crate::board;
use crate::video::Framebuffer;

const RUNS: usize = 5;
const FREQUENCY_SAMPLE_MICROSECONDS: u32 = 100_000;
const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
pub const CACHE_LINE: usize = 64;
/// Room for the largest offset, so a misaligned fill stays inside the framebuffer.
pub const SPARE_BYTES: usize = 64;

/// A dark gray in every pixel, so the screen stays dim. Each store repeats it.
const COLOR: u64 = 0xFF10_1010_FF10_1010;

// With the MMU on, the same physical RAM is reachable at these virtual offsets.
pub const NON_CACHEABLE_ALIAS: usize = 0x4000_0000;
pub const WRITE_BACK_ALIAS: usize = 0x8000_0000;

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
const SCTLR_ALIGNMENT_CHECK: u64 = 1 << 1;
const SCTLR_DATA_CACHE: u64 = 1 << 2;
const SCTLR_INSTRUCTION_CACHE: u64 = 1 << 12;
const ONE_GB: u64 = 0x4000_0000;
/// Lets EL1 and EL0 use the FPU and NEON without trapping.
const CPACR_FPEN: u64 = 0b11 << 20;
/// In DCZID_EL0: set if DC ZVA is prohibited, and the low bits of its block size.
const DCZID_PROHIBITED: u64 = 1 << 4;
const DCZID_SIZE_MASK: u64 = 0xF;
const DCZID_WORD_BYTES: usize = 4;

#[repr(C, align(4096))]
struct Table([u64; 512]);
static mut TABLE: Table = Table([0; 512]);

/// Identity-maps the first GB as cached and the top GB as device memory, and adds
/// the first GB again at two aliases with other memory types. Misaligned stores
/// stay allowed, as checking them would fault rather than show their cost.
pub fn enable_mmu() {
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
            "bic {value}, {value}, {check}",
            "orr {value}, {value}, {sctlr}",
            "msr sctlr_el1, {value}",
            "isb",
            mair = in(reg) MAIR,
            tcr = in(reg) TCR,
            base = in(reg) base,
            check = in(reg) SCTLR_ALIGNMENT_CHECK,
            sctlr = in(reg) sctlr,
            value = out(reg) _,
        );
    }
}

pub fn enable_fpu() {
    // SAFETY: only allows FPU instructions. The kernel's own code never uses them.
    unsafe {
        asm!(
            "mrs {value}, cpacr_el1",
            "orr {value}, {value}, {fpen}",
            "msr cpacr_el1, {value}",
            "isb",
            value = out(reg) _,
            fpen = in(reg) CPACR_FPEN,
        );
    }
}

/// The generic timer's count, once everything before it has finished.
pub fn ticks() -> u64 {
    let value: u64;
    // SAFETY: only reads the counter.
    unsafe { asm!("isb", "mrs {}, cntpct_el0", out(reg) value, options(nostack)) };
    value
}

fn finish_stores() {
    // SAFETY: only waits for earlier stores.
    unsafe { asm!("dsb sy", options(nostack)) };
}

/// Runs a store loop `count` times from `pointer`. `count` must not be 0, and
/// the caller keeps every iteration inside the framebuffer.
macro_rules! store_loop {
    ($pointer:ident, $count:ident, $($line:literal),+ $(,)?) => {
        // SAFETY: the stores stay in the region the caller passes.
        unsafe {
            asm!(
                ".p2align 6",
                "2:",
                $($line,)+
                "subs {count}, {count}, #1",
                "b.ne 2b",
                pointer = inout(reg) $pointer => _,
                count = inout(reg) $count => _,
                value = in(reg) COLOR,
                options(nostack),
            );
        }
    };
}

pub fn words(pointer: usize, count: usize) {
    store_loop!(pointer, count, "str {value:w}, [{pointer}], #4");
}

fn doubles(pointer: usize, count: usize) {
    store_loop!(pointer, count, "str {value}, [{pointer}], #8");
}

fn pairs_32(pointer: usize, count: usize) {
    store_loop!(pointer, count, "stp {value}, {value}, [{pointer}]", "stp {value}, {value}, [{pointer}, #16]", "add {pointer}, {pointer}, #32");
}

pub fn pairs_64(pointer: usize, count: usize) {
    store_loop!(
        pointer, count,
        "stp {value}, {value}, [{pointer}]",
        "stp {value}, {value}, [{pointer}, #16]",
        "stp {value}, {value}, [{pointer}, #32]",
        "stp {value}, {value}, [{pointer}, #48]",
        "add {pointer}, {pointer}, #64",
    );
}

fn pairs_48(pointer: usize, count: usize) {
    store_loop!(
        pointer, count,
        "stp {value}, {value}, [{pointer}]",
        "stp {value}, {value}, [{pointer}, #16]",
        "stp {value}, {value}, [{pointer}, #32]",
        "add {pointer}, {pointer}, #48",
    );
}

/// 11 32-bit registers per iteration, like `stmia r0!, {...}` with 11 registers.
fn registers_11(pointer: usize, count: usize) {
    store_loop!(
        pointer, count,
        "stp {value:w}, {value:w}, [{pointer}]",
        "stp {value:w}, {value:w}, [{pointer}, #8]",
        "stp {value:w}, {value:w}, [{pointer}, #16]",
        "stp {value:w}, {value:w}, [{pointer}, #24]",
        "stp {value:w}, {value:w}, [{pointer}, #32]",
        "str {value:w}, [{pointer}, #40]",
        "add {pointer}, {pointer}, #44",
    );
}

/// 13 32-bit registers per iteration, like `stmia r0!, {...}` with 13 registers.
pub fn registers_13(pointer: usize, count: usize) {
    store_loop!(
        pointer, count,
        "stp {value:w}, {value:w}, [{pointer}]",
        "stp {value:w}, {value:w}, [{pointer}, #8]",
        "stp {value:w}, {value:w}, [{pointer}, #16]",
        "stp {value:w}, {value:w}, [{pointer}, #24]",
        "stp {value:w}, {value:w}, [{pointer}, #32]",
        "stp {value:w}, {value:w}, [{pointer}, #40]",
        "str {value:w}, [{pointer}, #48]",
        "add {pointer}, {pointer}, #52",
    );
}

fn neon_64(pointer: usize, count: usize) {
    // SAFETY: the stores stay in the region the caller passes. The kernel is built
    // with no FPU code, so nothing else relies on v0.
    unsafe {
        asm!(
            ".arch_extension fp",
            ".arch_extension simd",
            "dup v0.2d, {value}",
            ".p2align 6",
            "2:",
            "stp q0, q0, [{pointer}]",
            "stp q0, q0, [{pointer}, #32]",
            "add {pointer}, {pointer}, #64",
            "subs {count}, {count}, #1",
            "b.ne 2b",
            pointer = inout(reg) pointer => _,
            count = inout(reg) count => _,
            value = in(reg) COLOR,
            options(nostack),
        );
    }
}

fn zero_blocks(mut pointer: usize, count: usize, block: usize) {
    for _ in 0..count {
        // SAFETY: inside the region the caller passes, which is normal memory.
        unsafe { asm!("dc zva, {}", in(reg) pointer, options(nostack)) };
        pointer += block;
    }
}

struct Strategy {
    name: &'static str,
    chunk: usize,
    /// Device memory faults on stores misaligned for their own size.
    store_size: usize,
    /// Whether to also try this one misaligned.
    try_offsets: bool,
    fill: fn(usize, usize),
}

const STRATEGIES: [Strategy; 8] = [
    Strategy { name: "STR W 4", chunk: 4, store_size: 4, try_offsets: false, fill: words },
    Strategy { name: "STR X 8", chunk: 8, store_size: 8, try_offsets: false, fill: doubles },
    Strategy { name: "STP X 32", chunk: 32, store_size: 8, try_offsets: true, fill: pairs_32 },
    Strategy { name: "STP X 64", chunk: 64, store_size: 8, try_offsets: true, fill: pairs_64 },
    Strategy { name: "STP Q 64", chunk: 64, store_size: 16, try_offsets: true, fill: neon_64 },
    Strategy { name: "STP X 48", chunk: 48, store_size: 8, try_offsets: true, fill: pairs_48 },
    Strategy { name: "11 regs 44", chunk: 44, store_size: 4, try_offsets: true, fill: registers_11 },
    Strategy { name: "13 regs 52", chunk: 52, store_size: 4, try_offsets: true, fill: registers_13 },
];
const OFFSETS: [usize; 3] = [0, 4, 8];
const FASTEST: usize = 0;
const MEDIAN: usize = RUNS / 2;

/// Sizes that fit the 32 KB L1 and the 1 MB shared L2 caches, filled repeatedly.
const CACHE_TEST_SIZES: [usize; 2] = [16 * 1024, 256 * 1024];
/// How much each of those tests writes in total, for the timer's sake.
const CACHE_TEST_TOTAL: usize = 8 * 1024 * 1024;
/// Unused RAM, to compare with the framebuffer.
const PLAIN_RAM: usize = 0x1000_0000;
const RAW_RUNS: usize = 12;
const RAW_PER_LINE: usize = 6;

const PROBE_SIZE: usize = 16 * 1024;
const PROBE_PASSES: usize = 512;
pub const PAIR_BYTES: usize = 16;
/// A simple loop that takes about one cycle per iteration, to estimate the clock.
const SPIN_LOOPS: u64 = 100_000_000;
/// In CPUECTLR_EL1: without this, the core's data caching isn't in effect.
const CPUECTLR_SMPEN: u64 = 1 << 6;

#[derive(Clone, Copy, PartialEq)]
pub enum Memory {
    Device,
    NonCacheable,
    WriteBack,
}

pub struct Target {
    pub label: &'static str,
    pub memory: Memory,
    pub base: usize,
    pub bytes: usize,
    pub frequency: u64,
}

pub fn clean(base: usize, bytes: usize, operation_is_clean_only: bool) {
    let mut line = base & !(CACHE_LINE - 1);
    while line < base + bytes {
        // SAFETY: only affects the cache's copy of this address, which is mapped.
        unsafe {
            if operation_is_clean_only {
                asm!("dc cvac, {}", in(reg) line, options(nostack));
            } else {
                asm!("dc civac, {}", in(reg) line, options(nostack));
            }
        }
        line += CACHE_LINE;
    }
    finish_stores();
}

/// Reports the runs' fastest and median, with the speed from the median.
pub fn report(name: &str, offset: usize, target: &Target, written: usize, runs: &mut [u64; RUNS]) {
    runs.sort_unstable();
    let (fastest, median) = (runs[FASTEST], runs[MEDIAN]);
    let speed = written as u64 * target.frequency / median.max(1) / MICROSECONDS_PER_SECOND;
    println!("{name:10} +{offset} {:3} {fastest:>9} {median:>9} {speed:>6}", target.label);
}

/// Times a fill, starting each run with the cache empty of this buffer.
pub fn time(target: &Target, mut fill: impl FnMut()) -> [u64; RUNS] {
    let mut runs = [0; RUNS];
    for run in &mut runs {
        if target.memory == Memory::WriteBack {
            clean(target.base, target.bytes, false);
        }
        let begin = ticks();
        fill();
        finish_stores();
        *run = ticks() - begin;
    }
    runs
}

fn run_stores(target: &Target) {
    for strategy in &STRATEGIES {
        for offset in OFFSETS {
            if offset != 0 && !strategy.try_offsets {
                continue;
            }
            if target.memory == Memory::Device && offset % strategy.store_size != 0 {
                continue;
            }
            let count = (target.bytes - SPARE_BYTES) / strategy.chunk;
            let pointer = target.base + offset;
            let mut runs = time(target, || (strategy.fill)(pointer, count));
            report(strategy.name, offset, target, count * strategy.chunk, &mut runs);
        }
    }
}

fn run_zero(target: &Target) {
    let id: u64;
    // SAFETY: only reads an ID register.
    unsafe { asm!("mrs {}, dczid_el0", out(reg) id, options(nostack)) };
    if id & DCZID_PROHIBITED != 0 {
        println!("DC ZVA is prohibited");
        return;
    }
    let block = DCZID_WORD_BYTES << (id & DCZID_SIZE_MASK);
    let count = target.bytes / block;
    let mut runs = time(target, || zero_blocks(target.base, count, block));
    report("DC ZVA", 0, target, count * block, &mut runs);
}

/// What handing a frame to the GPU costs, after the fill that left it dirty.
pub fn run_clean(target: &Target, name: &str, mut fill: impl FnMut()) {
    let mut runs = [0; RUNS];
    for run in &mut runs {
        clean(target.base, target.bytes, false);
        fill();
        finish_stores();
        let begin = ticks();
        clean(target.base, target.bytes, true);
        *run = ticks() - begin;
    }
    report(name, 0, target, target.bytes, &mut runs);
}

/// Fills that fit in the cache, with the buffer already in it, so nothing needs
/// to reach memory until the end. Shows the core's own speed for each store shape.
fn run_cache_sizes(target: &Target) {
    for size in CACHE_TEST_SIZES {
        println!("--- write-back, {} KB repeatedly", size / 1024);
        for strategy in &STRATEGIES {
            for offset in [0, 4] {
                if offset != 0 && !strategy.try_offsets {
                    continue;
                }
                let count = (size - SPARE_BYTES) / strategy.chunk;
                let passes = CACHE_TEST_TOTAL / size;
                let pointer = target.base + offset;
                let small = Target { base: target.base, bytes: size, label: target.label, memory: Memory::NonCacheable, frequency: target.frequency };
                (strategy.fill)(pointer, count);
                let mut runs = time(&small, || {
                    for _ in 0..passes {
                        (strategy.fill)(pointer, count);
                    }
                });
                report(strategy.name, offset, &small, count * strategy.chunk * passes, &mut runs);
            }
        }
        let dirty = Target { base: target.base, bytes: size, label: target.label, memory: Memory::WriteBack, frequency: target.frequency };
        run_clean(&dirty, "clean none", || {});
        run_clean(&dirty, "clean STP", || pairs_64(target.base, (size - SPARE_BYTES) / CACHE_LINE));
    }
    clean(target.base, target.bytes, false);
}

pub fn read_pairs(mut pointer: usize, count: usize) {
    for _ in 0..count {
        // SAFETY: inside the region the caller passes.
        unsafe { asm!("ldp {a}, {b}, [{p}]", a = out(reg) _, b = out(reg) _, p = in(reg) pointer, options(nostack, readonly)) };
        pointer += PAIR_BYTES;
    }
}

pub fn sparse_words(pointer: usize, count: usize) {
    store_loop!(pointer, count, "str {value:w}, [{pointer}], #64");
}

/// Separates "the cache is off" from "these writes bypass the cache": reads
/// can't bypass it, and neither can writes that don't fill whole lines.
fn run_cache_probes(non_cacheable: &Target, write_back: &Target) {
    let begin = ticks();
    let mut count = SPIN_LOOPS;
    // SAFETY: only counts down.
    unsafe { asm!(".p2align 6", "2:", "subs {0}, {0}, #1", "b.ne 2b", inout(reg) count, options(nostack)) };
    let _ = count;
    let spin = ticks() - begin;
    let megahertz = SPIN_LOOPS * write_back.frequency / spin.max(1) / MICROSECONDS_PER_SECOND;
    println!("--- about {megahertz} MHz, from a spin loop");

    let cases = [("nc", non_cacheable.base), ("wb", write_back.base)];
    for (label, base) in cases {
        let target = Target { label, memory: Memory::NonCacheable, base, bytes: PROBE_SIZE, frequency: write_back.frequency };
        read_pairs(base, PROBE_SIZE / PAIR_BYTES);
        let mut runs = time(&target, || {
            for _ in 0..PROBE_PASSES {
                read_pairs(base, PROBE_SIZE / PAIR_BYTES);
            }
        });
        report("read 16K", 0, &target, PROBE_SIZE * PROBE_PASSES, &mut runs);

        sparse_words(base, PROBE_SIZE / CACHE_LINE);
        let mut runs = time(&target, || {
            for _ in 0..PROBE_PASSES {
                sparse_words(base, PROBE_SIZE / CACHE_LINE);
            }
        });
        report("sparse", 0, &target, PROBE_SIZE / CACHE_LINE * DCZID_WORD_BYTES * PROBE_PASSES, &mut runs);
    }

    let extended: u64;
    // SAFETY: reads a register. If this level isn't allowed to, it traps, which
    // the experiment reports by never finishing, with the results above intact.
    unsafe { asm!("mrs {}, s3_1_c15_c2_1", out(reg) extended, options(nostack)) };
    println!("CPUECTLR {extended:#x}, SMPEN {}", extended & CPUECTLR_SMPEN != 0);
}

/// Prints every run's ticks, in order, for one fill of 64 bytes at a time.
fn run_raw(name: &str, base: usize, bytes: usize) {
    let count = (bytes - SPARE_BYTES) / CACHE_LINE;
    let mut runs = [0; RAW_RUNS];
    for run in &mut runs {
        let begin = ticks();
        pairs_64(base, count);
        finish_stores();
        *run = ticks() - begin;
    }
    println!("{name}:");
    for row in runs.chunks(RAW_PER_LINE) {
        println!(" {row:?}");
    }
}

pub fn measure_frequency() -> u64 {
    let start_time = board::microseconds();
    let start_ticks = ticks();
    while board::microseconds().wrapping_sub(start_time) < FREQUENCY_SAMPLE_MICROSECONDS {}
    let elapsed = u64::from(board::microseconds().wrapping_sub(start_time));
    (ticks() - start_ticks) * MICROSECONDS_PER_SECOND / elapsed
}

pub fn run(framebuffer: &Framebuffer) -> ! {
    let bytes = framebuffer.pitch * framebuffer.height;
    let screen = framebuffer.base;
    let frequency = measure_frequency();
    let reported: u64;
    let sctlr: u64;
    // SAFETY: only reads system registers.
    unsafe {
        asm!("mrs {}, cntfrq_el0", out(reg) reported, options(nostack));
        asm!("mrs {}, sctlr_el1", out(reg) sctlr, options(nostack));
    }
    enable_fpu();

    println!("--- screen {screen:#x}, {bytes} bytes");
    println!("timer {frequency} Hz measured, {reported} reported, SCTLR {sctlr:#x}");
    println!("strategy  off mem       min    median   MB/s");

    run_raw("dev ram", PLAIN_RAM, bytes);
    run_raw("dev screen", screen, bytes);
    run_stores(&Target { label: "dev", memory: Memory::Device, base: screen, bytes, frequency });

    enable_mmu();
    let non_cacheable =
        Target { label: "nc", memory: Memory::NonCacheable, base: screen + NON_CACHEABLE_ALIAS, bytes, frequency };
    run_stores(&non_cacheable);
    run_zero(&non_cacheable);

    let write_back =
        Target { label: "wb", memory: Memory::WriteBack, base: screen + WRITE_BACK_ALIAS, bytes, frequency };
    run_stores(&write_back);
    run_zero(&write_back);

    let lines = (bytes - SPARE_BYTES) / CACHE_LINE;
    run_clean(&write_back, "clean none", || {});
    run_clean(&write_back, "clean STP", || pairs_64(write_back.base, lines));
    run_clean(&write_back, "clean ZVA", || zero_blocks(write_back.base, bytes / CACHE_LINE, CACHE_LINE));

    run_cache_sizes(&write_back);
    run_cache_probes(&non_cacheable, &write_back);
    println!("--- plain RAM");
    run_raw("nc ram", PLAIN_RAM + NON_CACHEABLE_ALIAS, bytes);
    run_raw("nc screen", non_cacheable.base, bytes);

    clean(write_back.base, bytes, true);
    println!("DONE");
    loop {
        // SAFETY: only waits for an event.
        unsafe { asm!("wfe", options(nostack)) };
    }
}
