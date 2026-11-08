//! Throwaway kernel module: times drawing each version of Aki, at the firmware's
//! default CPU clock and at its maximum. See run.py in this folder. The MMU stays off,
//! as it is in the real kernel.

use core::arch::asm;

use crate::assets::{self, Assets};
use crate::board;
use crate::clock;
use crate::image;
use crate::video::{Framebuffer, BLACK};

const RUNS: usize = 5;
const MEDIAN: usize = RUNS / 2;
const MEGAHERTZ: u32 = 1_000_000;
const MICROSECONDS_PER_MILLISECOND: u32 = 1000;
const AKI_LEFT: usize = 1152;
const AKI_TOP: usize = 0;

/// Name, body and mouth slots, and the mouth's position, as in `pictures.rs`.
const VERSIONS: [(&str, usize, usize, usize, usize); 4] = [
    ("original", assets::AKI_ORIGINAL, assets::AKI_ORIGINAL_MOUTH, 368, 220),
    ("x1", assets::AKI, assets::AKI_MOUTH, 368, 220),
    ("x2", assets::AKI_X2, assets::AKI_X2_MOUTH, 368, 220),
    ("x4", assets::AKI_X4, assets::AKI_X4_MOUTH, 368, 220),
];

fn report(name: &str, runs: &mut [u32; RUNS]) {
    runs.sort_unstable();
    let (fastest, median) = (runs[0], runs[MEDIAN]);
    println!("{name:9} min {:>4} ms, median {:>4} ms", fastest / MICROSECONDS_PER_MILLISECOND, median / MICROSECONDS_PER_MILLISECOND);
}

pub fn time_clear(framebuffer: &Framebuffer) {
    let mut runs = [0; RUNS];
    for run in &mut runs {
        let start = board::microseconds();
        framebuffer.fill(BLACK);
        *run = board::microseconds().wrapping_sub(start);
    }
    report("clear", &mut runs);
}

pub fn time_versions(framebuffer: &Framebuffer, assets: &mut Assets) {
    for (name, body, mouth, left, top) in VERSIONS {
        let (Some(body), Some(mouth)) = (assets.get(body), assets.get(mouth)) else {
            println!("{name}: not in memory");
            continue;
        };
        let mut runs = [0; RUNS];
        let mut drawn = true;
        for run in &mut runs {
            framebuffer.fill(BLACK);
            let start = board::microseconds();
            drawn &= image::draw(framebuffer, AKI_LEFT, AKI_TOP, body).is_ok();
            drawn &= image::draw(framebuffer, AKI_LEFT + left, AKI_TOP + top, mouth).is_ok();
            *run = board::microseconds().wrapping_sub(start);
        }
        if !drawn {
            println!("{name}: couldn't draw");
        }
        report(name, &mut runs);
    }
}

pub fn run(framebuffer: &Framebuffer) -> ! {
    let low = clock::get(clock::VIEW, clock::TAG_GET_RATE, clock::CLOCK_ARM);
    let high = clock::get(clock::VIEW, clock::TAG_GET_MAX_RATE, clock::CLOCK_ARM);
    println!("=== ARM {} MHz, max {}", low / MEGAHERTZ, high / MEGAHERTZ);
    match Assets::find(board::assets_region(), crate::card::image_area()) {
        Some(mut assets) if low != 0 && high != 0 => {
            for rate in [low, high, low] {
                clock::set_arm_rate(clock::VIEW, rate);
                let measured = clock::get(clock::VIEW, clock::TAG_GET_MEASURED_RATE, clock::CLOCK_ARM) / MEGAHERTZ;
                let core = clock::get(clock::VIEW, clock::TAG_GET_MEASURED_RATE, clock::CLOCK_CORE) / MEGAHERTZ;
                let degrees = clock::get(clock::VIEW, clock::TAG_GET_TEMPERATURE, clock::TEMPERATURE_SENSOR) / 1000;
                println!("--- ARM {measured} MHz, core {core} MHz, {degrees} C");
                time_clear(framebuffer);
                time_versions(framebuffer, &mut assets);
            }
        }
        _ => println!("no images, or no answer from the firmware"),
    }
    println!("DONE");
    loop {
        // SAFETY: only waits for an event.
        unsafe { asm!("wfe", options(nostack)) };
    }
}
