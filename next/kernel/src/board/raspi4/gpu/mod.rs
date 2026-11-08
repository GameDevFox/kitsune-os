//! The 3D core, V3D 4.2. Broadcom publishes no manual for it, so this follows Linux's v3d
//! driver, Mesa's `src/broadcom`, and the bare-metal example at
//! github.com/Random06457/rpi4-gpu-bare-metal-examples (MIT).

use core::ptr::{read_volatile, write_volatile};

use super::{microseconds, PERIPHERAL_BASE, UNCACHED_VIEW};
use crate::video::Framebuffer;

mod commands;
mod shaders;

use commands::{Lists, TILE_ALLOCATION_BYTES};
pub use commands::{HEIGHT, WIDTH};

const PM_GRAFX: usize = PERIPHERAL_BASE + 0x10_010C;
const PM_PASSWORD: u32 = 0x5A00_0000;
/// In `PM_GRAFX`: releases the V3D from reset.
const PM_V3D_RESET_RELEASE: u32 = 1 << 6;

const ASB_V3D_SLAVE: usize = PERIPHERAL_BASE + 0xC1_1008;
const ASB_V3D_MASTER: usize = PERIPHERAL_BASE + 0xC1_100C;
const ASB_REQUEST_STOP: u32 = 1 << 0;
const ASB_ACKNOWLEDGE: u32 = 1 << 1;

const HUB: usize = PERIPHERAL_BASE + 0xC0_0000;
const HUB_IDENT0: usize = HUB + 0x08;
const HUB_IDENT1: usize = HUB + 0x0C;
const HUB_IDENT2: usize = HUB + 0x10;
const CORE0: usize = PERIPHERAL_BASE + 0xC0_4000;
const CORE_IDENT0: usize = CORE0;
const CORE_IDENT1: usize = CORE0 + 0x04;

const CTL_SLCACTL: usize = CORE0 + 0x24;
const CTL_L2TCACTL: usize = CORE0 + 0x30;
const CTL_L2TFLSTA: usize = CORE0 + 0x34;
const CTL_L2TFLEND: usize = CORE0 + 0x38;
const L2T_FLUSH: u32 = 1 << 0;
const FLUSH_ALL: u32 = u32::MAX;

// The control list executor: thread 0 bins, and thread 1 renders.
const CLE_CT0CS: usize = CORE0 + 0x100;
const CLE_CT1CS: usize = CORE0 + 0x104;
const CLE_CT0EA: usize = CORE0 + 0x108;
const CLE_CT1EA: usize = CORE0 + 0x10C;
const CLE_CT0CA: usize = CORE0 + 0x110;
const CLE_CT1CA: usize = CORE0 + 0x114;
const CLE_BFC: usize = CORE0 + 0x134;
const CLE_RFC: usize = CORE0 + 0x138;
const CLE_CT0QTS: usize = CORE0 + 0x15C;
const CLE_CT0QBA: usize = CORE0 + 0x160;
const CLE_CT1QBA: usize = CORE0 + 0x164;
const CLE_CT0QEA: usize = CORE0 + 0x168;
const CLE_CT1QEA: usize = CORE0 + 0x16C;
const CLE_CT0QMA: usize = CORE0 + 0x170;
const CLE_CT0QMS: usize = CORE0 + 0x174;
const CT0QTS_ENABLE: u32 = 1 << 1;
const COUNT_MASK: u32 = 0xFF;

const POWER_TIMEOUT_MICROSECONDS: u32 = 100_000;
const JOB_TIMEOUT_MICROSECONDS: u32 = 500_000;

/// Unused RAM for everything the GPU reads and writes. The GPU uses these physical
/// addresses, and can't see into the CPU's cache, so the CPU uses the uncached view.
const MEMORY_BASE: usize = 0x1000_0000;
const MEMORY_BYTES: usize = 16 * 1024 * 1024;

// IEEE floats, as the kernel has none of its own and the GPU only reads their bits.
const FLOAT_ZERO: u32 = 0;
const FLOAT_HALF: u32 = 0x3F00_0000;
const FLOAT_ONE: u32 = 0x3F80_0000;

/// Converts a whole number to the bits of the same float.
const fn float_bits(value: i32) -> u32 {
    fixed_bits(value, 0)
}

/// Converts a number with this many fractional bits to the bits of the same float.
pub const fn fixed_bits(value: i32, fraction_bits: u32) -> u32 {
    if value == 0 {
        return FLOAT_ZERO;
    }
    const MANTISSA_BITS: u32 = 23;
    const EXPONENT_BIAS: u32 = 127;
    const SIGN: u32 = 1 << 31;
    let sign = if value < 0 { SIGN } else { 0 };
    let magnitude = value.unsigned_abs();
    let top_bit = u32::BITS - 1 - magnitude.leading_zeros();
    let fraction = if top_bit > MANTISSA_BITS { magnitude >> (top_bit - MANTISSA_BITS) } else { magnitude << (MANTISSA_BITS - top_bit) };
    sign | (EXPONENT_BIAS + top_bit - fraction_bits) << MANTISSA_BITS | (fraction & ((1 << MANTISSA_BITS) - 1))
}

fn read(address: usize) -> u32 {
    // SAFETY: only used on this board's power manager and, once it's on, GPU registers.
    unsafe { read_volatile(address as *const u32) }
}

fn write(address: usize, value: u32) {
    // SAFETY: as in `read`.
    unsafe { write_volatile(address as *mut u32, value) };
}

/// Lets a bus bridge run. Its acknowledge bit clears once it has.
fn start_bridge(control: usize) -> bool {
    write(control, PM_PASSWORD | (read(control) & !ASB_REQUEST_STOP));
    let start = microseconds();
    while read(control) & ASB_ACKNOWLEDGE != 0 {
        if microseconds().wrapping_sub(start) > POWER_TIMEOUT_MICROSECONDS {
            return false;
        }
    }
    true
}

/// Powers the GPU on, and prints what it says it is. Returns false if it doesn't answer,
/// without touching its registers, as reading them while it's off could hang the bus.
pub fn power_on() -> bool {
    write(PM_GRAFX, PM_PASSWORD | read(PM_GRAFX) | PM_V3D_RESET_RELEASE);
    if !start_bridge(ASB_V3D_MASTER) {
        println!("GPU: the master bridge didn't start.");
        return false;
    }
    if !start_bridge(ASB_V3D_SLAVE) {
        println!("GPU: the slave bridge didn't start.");
        return false;
    }
    println!(
        "GPU: hub {:#010x} {:#010x} {:#010x}, core {:#010x} {:#010x}.",
        read(HUB_IDENT0),
        read(HUB_IDENT1),
        read(HUB_IDENT2),
        read(CORE_IDENT0),
        read(CORE_IDENT1)
    );
    true
}

fn invalidate_caches() {
    write(CTL_L2TFLSTA, 0);
    write(CTL_L2TFLEND, FLUSH_ALL);
    write(CTL_L2TCACTL, L2T_FLUSH);
    write(CTL_SLCACTL, FLUSH_ALL);
}

fn print_state() {
    println!(
        "GPU: bin {:#x} at {:#x} of {:#x}, render {:#x} at {:#x} of {:#x}, counts {} {}.",
        read(CLE_CT0CS),
        read(CLE_CT0CA),
        read(CLE_CT0EA),
        read(CLE_CT1CS),
        read(CLE_CT1CA),
        read(CLE_CT1EA),
        read(CLE_BFC) & COUNT_MASK,
        read(CLE_RFC) & COUNT_MASK
    );
}

/// Waits for a job counter to move on from `before`.
fn wait_for_count(counter: usize, before: u32, what: &str) -> bool {
    let start = microseconds();
    while read(counter) & COUNT_MASK == before {
        if microseconds().wrapping_sub(start) > JOB_TIMEOUT_MICROSECONDS {
            println!("GPU: timed out {what}.");
            print_state();
            return false;
        }
    }
    true
}

const POSITION_WORDS: usize = 3;
const WORD_BYTES: usize = 4;
const COLOR_BYTES: usize = 4;

/// Where to draw, in pixels on the screen.
const TOP_LEFT_BYTES_PER_PIXEL: usize = 4;

/// A vertex, as the bits of three floats from -1 to 1, with x right, y up, and smaller z
/// nearer, and a color as red, green, blue, alpha.
pub struct Vertex {
    pub position: [u32; POSITION_WORDS],
    pub color: [u8; COLOR_BYTES],
}

/// Built lists, ready to draw a set number of vertices at one place on the screen.
pub struct Scene {
    lists: Lists,
}

impl Scene {
    /// Powers on the GPU, and prepares to draw `vertex_count` vertices with the image's top
    /// left corner at `left`, `top`. Returns `None` if the GPU doesn't answer.
    pub fn new(framebuffer: &Framebuffer, left: usize, top: usize, vertex_count: u32) -> Option<Self> {
        if !power_on() {
            return None;
        }
        assert!(left + WIDTH <= framebuffer.width && top + HEIGHT <= framebuffer.height);
        let target = framebuffer.base - UNCACHED_VIEW + top * framebuffer.pitch + left * TOP_LEFT_BYTES_PER_PIXEL;
        Some(Self { lists: commands::build(target as u32, framebuffer.pitch as u32, vertex_count) })
    }

    pub fn set_vertex(&self, index: usize, vertex: &Vertex) {
        let position = self.lists.positions as usize + UNCACHED_VIEW + index * POSITION_WORDS * WORD_BYTES;
        let color = self.lists.colors as usize + UNCACHED_VIEW + index * COLOR_BYTES;
        for (word, bits) in vertex.position.iter().enumerate() {
            // SAFETY: inside the attributes `Scene::new` reserved, if `index` is in range.
            unsafe { write_volatile((position + word * WORD_BYTES) as *mut u32, *bits) };
        }
        // SAFETY: as above.
        unsafe { write_volatile(color as *mut u32, u32::from_le_bytes(vertex.color)) };
    }

    /// Draws a frame, and waits for it. Returns false if the GPU timed out, having said why.
    pub fn render(&self) -> bool {
        // The GPU reads this memory directly, so the CPU's writes must have reached it.
        // SAFETY: only waits for earlier writes.
        unsafe { core::arch::asm!("dsb sy", options(nostack)) };
        invalidate_caches();
        let before = read(CLE_BFC) & COUNT_MASK;
        write(CLE_CT0QMA, self.lists.tile_allocation);
        write(CLE_CT0QMS, TILE_ALLOCATION_BYTES as u32);
        write(CLE_CT0QTS, self.lists.tile_state | CT0QTS_ENABLE);
        write(CLE_CT0QBA, self.lists.bin_start);
        write(CLE_CT0QEA, self.lists.bin_end);
        if !wait_for_count(CLE_BFC, before, "binning") {
            return false;
        }
        invalidate_caches();
        let before = read(CLE_RFC) & COUNT_MASK;
        write(CLE_CT1QBA, self.lists.render_start);
        write(CLE_CT1QEA, self.lists.render_end);
        wait_for_count(CLE_RFC, before, "rendering")
    }
}
