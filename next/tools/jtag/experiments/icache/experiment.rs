//! Throwaway kernel module: compares drawing with the instruction cache off and on,
//! in one boot, at whatever clock the kernel set. See run.py in this folder. Also
//! times decoding alone, and logs temperature during sustained drawing.

use core::arch::asm;

use crate::assets::{self, Assets};
use crate::board;
use crate::clock;
use crate::draw;
use crate::image;
use crate::video::{Framebuffer, BLACK};

const SCTLR_INSTRUCTION_CACHE: u64 = 1 << 12;
const MEGAHERTZ: u32 = 1_000_000;
const SPIN_LOOPS: u64 = 10_000_000;
const RUNS: usize = 5;
const MEDIAN: usize = RUNS / 2;
const MICROSECONDS_PER_MILLISECOND: u32 = 1000;
/// Past the screen's right edge, so a draw decodes everything and writes nothing.
const OFF_SCREEN_LEFT: usize = 1920;
const AKI_LEFT: usize = 1152;
const SOAK_REPORTS: u32 = 4;
const SOAK_REPORT_MICROSECONDS: u32 = 30_000_000;
const THROTTLED_TAG: u32 = 0x0003_0046;

fn set_instruction_cache(on: bool) {
    // SAFETY: invalidating is always safe here, as the code in RAM hasn't changed,
    // and this bit only decides whether fetches may be cached.
    unsafe {
        asm!(
            "ic iallu",
            "dsb ish",
            "isb",
            "mrs {value}, sctlr_el1",
            "bic {value}, {value}, {bit}",
            "orr {value}, {value}, {wanted}",
            "msr sctlr_el1, {value}",
            "isb",
            value = out(reg) _,
            bit = in(reg) SCTLR_INSTRUCTION_CACHE,
            wanted = in(reg) if on { SCTLR_INSTRUCTION_CACHE } else { 0 },
            options(nostack),
        );
    }
}

fn spin_speed() -> u32 {
    let start = board::microseconds();
    let mut count = SPIN_LOOPS;
    // SAFETY: only counts down.
    unsafe { asm!(".p2align 6", "2:", "subs {0}, {0}, #1", "b.ne 2b", inout(reg) count, options(nostack)) };
    let _ = count;
    let elapsed = u64::from(board::microseconds().wrapping_sub(start)).max(1);
    (SPIN_LOOPS * u64::from(MEGAHERTZ) / elapsed / u64::from(MEGAHERTZ)) as u32
}

fn time_decoding(framebuffer: &Framebuffer, assets: &mut Assets) {
    let slots = [("original", assets::AKI_ORIGINAL), ("x1", assets::AKI), ("x2", assets::AKI_X2), ("x4", assets::AKI_X4)];
    for (name, slot) in slots {
        let Some(data) = assets.get(slot) else {
            continue;
        };
        let mut runs = [0; RUNS];
        for run in &mut runs {
            let start = board::microseconds();
            let _ = image::draw(framebuffer, OFF_SCREEN_LEFT, 0, data);
            *run = board::microseconds().wrapping_sub(start);
        }
        runs.sort_unstable();
        println!("decode {name:9} min {:>4} ms, median {:>4} ms", runs[0] / MICROSECONDS_PER_MILLISECOND, runs[MEDIAN] / MICROSECONDS_PER_MILLISECOND);
    }
}

fn phase(framebuffer: &Framebuffer, assets: &mut Assets, on: bool) {
    set_instruction_cache(on);
    println!("--- instruction cache {}, spin {} M/s", if on { "ON" } else { "off" }, spin_speed());
    draw::time_clear(framebuffer);
    draw::time_versions(framebuffer, assets);
    time_decoding(framebuffer, assets);
}

fn soak(framebuffer: &Framebuffer, assets: &mut Assets) {
    let Some(body) = assets.get(assets::AKI) else {
        return;
    };
    for report in 1..=SOAK_REPORTS {
        let start = board::microseconds();
        let mut draws = 0;
        while board::microseconds().wrapping_sub(start) < SOAK_REPORT_MICROSECONDS {
            framebuffer.fill(BLACK);
            let _ = image::draw(framebuffer, AKI_LEFT, 0, body);
            draws += 1;
        }
        let measured = clock::get(clock::VIEW, clock::TAG_GET_MEASURED_RATE, clock::CLOCK_ARM) / MEGAHERTZ;
        let degrees = clock::get(clock::VIEW, clock::TAG_GET_TEMPERATURE, clock::TEMPERATURE_SENSOR) / 1000;
        let throttled = clock::get(clock::VIEW, THROTTLED_TAG, 0);
        println!("soak {report}: {draws} draws, ARM {measured} MHz, {degrees} C, throttled {throttled:#x}");
    }
}

pub fn run(framebuffer: &Framebuffer) -> ! {
    let measured = clock::get(clock::VIEW, clock::TAG_GET_MEASURED_RATE, clock::CLOCK_ARM) / MEGAHERTZ;
    let core = clock::get(clock::VIEW, clock::TAG_GET_MEASURED_RATE, clock::CLOCK_CORE) / MEGAHERTZ;
    let degrees = clock::get(clock::VIEW, clock::TAG_GET_TEMPERATURE, clock::TEMPERATURE_SENSOR) / 1000;
    println!("=== ARM {measured} MHz, core {core} MHz, {degrees} C");
    match Assets::find(board::assets_region(), crate::card::image_area()) {
        Some(mut assets) => {
            phase(framebuffer, &assets, false);
            phase(framebuffer, &assets, true);
            phase(framebuffer, &assets, false);
            set_instruction_cache(true);
            println!("--- two minutes of drawing, instruction cache on");
            soak(framebuffer, &mut assets);
        }
        None => println!("no images"),
    }
    println!("DONE");
    loop {
        // SAFETY: only waits for an event.
        unsafe { asm!("wfe", options(nostack)) };
    }
}
