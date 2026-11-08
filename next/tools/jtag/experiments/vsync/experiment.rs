//! Throwaway kernel module: measures flipping and vsync. See run.py in this folder.

use core::ptr::{read_volatile, write_volatile};

use crate::board;
use crate::board::mailbox::property;
use crate::video::Framebuffer;

const TAG_SET_VSYNC: u32 = 0x0004_800E;
const TAG_SET_VIRTUAL_OFFSET: u32 = 0x0004_8009;
const TAG_GET_VIRTUAL_OFFSET: u32 = 0x0004_0009;
const TAG_GET_VIRTUAL_SIZE: u32 = 0x0004_0004;

const HVS_BASE: usize = 0xFE40_0000;
const DISPSTAT0: usize = 0x48;
const DISPSTAT_STRIDE: usize = 0x10;
const LINE_MASK: u32 = 0xFFF;
const MODE_SHIFT: u32 = 30;
const MODE_RUN: u32 = 2;
const CHANNELS: usize = 3;

const SMI_BASE: usize = 0xFE60_0000;
const SMI_CS: *mut u32 = SMI_BASE as *mut u32;
const SMI_DSW0: *const u32 = (SMI_BASE + 0x14) as *const u32;
const SMI_INTERRUPTS: u32 = 0b111 << 9;

const VSYNC_SAMPLES: u32 = 120;
const FLIP_SAMPLES: u32 = 60;
const BETWEEN_FLIPS_US: u32 = 20_000;
const LINE_WATCH_US: u32 = 500_000;
const SMI_WATCH_US: u32 = 500_000;
const FREE_FLIP_US: u32 = 27_000;
/// Slow enough for a 30 fps capture to see each color, and not a whole number of frames.
const SLOW_FREE_FLIP_US: u32 = 251_000;
const SLOW_SYNC_FRAMES: u32 = 15;
/// A drop this large in the line counter means a new frame started.
const WRAP_DROP: u32 = 100;

const BLUE: u32 = 0xFF00_0040;
const RED: u32 = 0xFF40_0000;

#[derive(Default)]
struct Stats {
    count: u32,
    total: u64,
    low: u32,
    high: u32,
}

impl Stats {
    fn add(&mut self, value: u32) {
        self.low = if self.count == 0 { value } else { self.low.min(value) };
        self.high = self.high.max(value);
        self.total += u64::from(value);
        self.count += 1;
    }

    fn print(&self, name: &str) {
        let average = if self.count == 0 { 0 } else { self.total / u64::from(self.count) };
        println!("{name}: n={} min={} avg={average} max={} us", self.count, self.low, self.high);
    }
}

fn wait_vsync() -> bool {
    let mut values = [0];
    property(TAG_SET_VSYNC, &mut values)
}

fn set_offset(y: u32) -> (bool, u32) {
    let mut values = [0, y];
    let ok = property(TAG_SET_VIRTUAL_OFFSET, &mut values);
    (ok, values[1])
}

fn delay(microseconds: u32) {
    let start = board::microseconds();
    while board::microseconds().wrapping_sub(start) < microseconds {}
}

fn dispstat(channel: usize) -> u32 {
    let address = HVS_BASE + DISPSTAT0 + channel * DISPSTAT_STRIDE;
    // SAFETY: a status register in the HVS's address range, and only read.
    unsafe { read_volatile(address as *const u32) }
}

/// Returns whether the tag works at all. Runs last, because a call that times
/// out could leave a late reply behind.
fn measure_vsync() -> bool {
    println!("--- SET_VSYNC");
    let start = board::microseconds();
    let first = wait_vsync();
    println!("first call: ok={first} after {} us", board::microseconds().wrapping_sub(start));
    if !first {
        return false;
    }

    let mut stats = Stats::default();
    let mut failures = 0;
    let mut last = board::microseconds();
    for _ in 0..VSYNC_SAMPLES {
        if !wait_vsync() {
            failures += 1;
        }
        let now = board::microseconds();
        stats.add(now.wrapping_sub(last));
        last = now;
    }
    stats.print("return interval");
    println!("failures: {failures}");
    failures == 0
}

fn measure_flip(height: u32) {
    println!("--- SET_VIRTUAL_OFFSET");
    let mut stats = Stats::default();
    let mut failures = 0;
    let mut mismatches = 0;
    for index in 0..FLIP_SAMPLES {
        let y = if index % 2 == 0 { height } else { 0 };
        let start = board::microseconds();
        let (ok, got) = set_offset(y);
        stats.add(board::microseconds().wrapping_sub(start));
        failures += u32::from(!ok);
        mismatches += u32::from(got != y);
        delay(BETWEEN_FLIPS_US);
    }
    stats.print("call time");
    println!("failures: {failures} mismatched replies: {mismatches}");
    set_offset(0);
}

fn active_channel() -> Option<usize> {
    println!("--- HVS line counter");
    let mut active = None;
    for channel in 0..CHANNELS {
        let status = dispstat(channel);
        let mode = status >> MODE_SHIFT;
        println!("channel {channel}: status={status:#010x} mode={mode} line={}", status & LINE_MASK);
        if mode == MODE_RUN && active.is_none() {
            active = Some(channel);
        }
    }
    active
}

fn measure_lines(channel: usize) {
    let mut low = u32::MAX;
    let mut high = 0;
    let mut samples = 0u32;
    let mut stats = Stats::default();
    let mut previous = dispstat(channel) & LINE_MASK;
    let mut wrap_time = None;
    let start = board::microseconds();
    while board::microseconds().wrapping_sub(start) < LINE_WATCH_US {
        let line = dispstat(channel) & LINE_MASK;
        low = low.min(line);
        high = high.max(line);
        samples += 1;
        if line + WRAP_DROP < previous {
            let now = board::microseconds();
            if let Some(last) = wrap_time {
                stats.add(now.wrapping_sub(last));
            }
            wrap_time = Some(now);
        }
        previous = line;
    }
    println!("channel {channel}: {samples} reads, line range {low}-{high}");
    stats.print("frame interval");
}

fn wait_new_frame(channel: usize) {
    let mut previous = dispstat(channel) & LINE_MASK;
    loop {
        let line = dispstat(channel) & LINE_MASK;
        if line + WRAP_DROP < previous {
            return;
        }
        previous = line;
    }
}

fn measure_smi() {
    println!("--- SMI");
    // SAFETY: registers in the SMI block, which nothing else here uses.
    unsafe {
        println!("CS={:#010x} DSW0={:#010x}", read_volatile(SMI_CS), read_volatile(SMI_DSW0));
        let mut stats = Stats::default();
        let mut last = None;
        let start = board::microseconds();
        write_volatile(SMI_CS, 0);
        while board::microseconds().wrapping_sub(start) < SMI_WATCH_US {
            if read_volatile(SMI_CS) & SMI_INTERRUPTS != 0 {
                let now = board::microseconds();
                if let Some(previous) = last {
                    stats.add(now.wrapping_sub(previous));
                }
                last = Some(now);
                write_volatile(SMI_CS, 0);
            }
        }
        println!("interrupt seen: {}", last.is_some());
        stats.print("interrupt interval");
        println!("CS={:#010x} DSW0={:#010x}", read_volatile(SMI_CS), read_volatile(SMI_DSW0));
    }
}

/// How to time flips, chosen over serial.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Hold,
    Free,
    MailboxVsync,
    LineCounter,
    SlowFree,
    SlowLineCounter,
}

pub fn run(framebuffer: &Framebuffer) -> ! {
    let height = framebuffer.height as u32;
    let channel = active_channel();
    let vsync_works = false;

    framebuffer.fill(BLUE);
    let second = Framebuffer {
        base: framebuffer.base + framebuffer.height * framebuffer.pitch,
        width: framebuffer.width,
        height: framebuffer.height,
        pitch: framebuffer.pitch,
    };
    second.fill(RED);
    println!("READY");

    let mut mode = Mode::Hold;
    let mut showing_second = false;
    loop {
        match board::console_try_read() {
            Some(command @ (b'a' | b'b')) => {
                let y = if command == b'a' { 0 } else { height };
                let start = board::microseconds();
                let (ok, got) = set_offset(y);
                let took = board::microseconds().wrapping_sub(start);
                let mut now = [0, 0];
                let read_ok = property(TAG_GET_VIRTUAL_OFFSET, &mut now);
                let mut size = [0, 0];
                let size_ok = property(TAG_GET_VIRTUAL_SIZE, &mut size);
                println!(
                    "set y={y}: ok={ok} reply={got} took={took}us; get ok={read_ok} y={}; size ok={size_ok} {}x{}",
                    now[1], size[0], size[1]
                );
            }
            Some(b'h') => {
                mode = Mode::Hold;
                set_offset(0);
                showing_second = false;
            }
            Some(b'r') => measure_flip(height),
            Some(b'v') => {
                measure_vsync();
            }
            Some(b'i') => {
                if let Some(channel) = channel {
                    measure_lines(channel);
                }
                measure_smi();
            }
            Some(b'F') => mode = Mode::SlowFree,
            Some(b'L') => mode = Mode::SlowLineCounter,
            Some(b'f') => mode = Mode::Free,
            Some(b's') if vsync_works => mode = Mode::MailboxVsync,
            Some(b'l') => mode = Mode::LineCounter,
            _ => {}
        }

        match (mode, channel) {
            (Mode::Hold, _) => continue,
            (Mode::Free, _) => delay(FREE_FLIP_US),
            (Mode::MailboxVsync, _) => {
                wait_vsync();
            }
            (Mode::LineCounter, Some(channel)) => wait_new_frame(channel),
            (Mode::SlowFree, _) => delay(SLOW_FREE_FLIP_US),
            (Mode::SlowLineCounter, Some(channel)) => {
                for _ in 0..SLOW_SYNC_FRAMES {
                    wait_new_frame(channel);
                }
            }
            (Mode::LineCounter | Mode::SlowLineCounter, None) => continue,
        }
        showing_second = !showing_second;
        set_offset(if showing_second { height } else { 0 });
    }
}
