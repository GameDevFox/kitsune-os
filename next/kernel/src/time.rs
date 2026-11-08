//! Time, and alarms. Uptime comes from the ARM generic counter, 64 bits that won't wrap,
//! and alarms from the system timer's compare channels, interrupting through the GIC.
//! Nothing else uses either yet; `check` shows both work.
//!
//! The two counters tick at the same rate, but the ARM counter starts when the firmware
//! starts the ARM cores, about 6.97 s after the system timer starts at reset: measured
//! on 2026-11-07 as 6,965,125 us apart in one run and 6,965,126 us in another.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::arch::timer;
use crate::board::{self, system_timer::{SystemTimer, CHANNEL_1}};
use crate::devices;
use crate::interrupts;

const MICROSECONDS_PER_SECOND: u128 = 1_000_000;

/// Microseconds since the firmware started the ARM cores, by the ARM generic counter.
pub fn uptime_microseconds() -> u64 {
    let frequency = u128::from(timer::frequency()).max(1);
    (u128::from(timer::counter()) * MICROSECONDS_PER_SECOND / frequency) as u64
}

const ALARM_CHANNEL: board::system_timer::Channel = CHANNEL_1;
const ALARM_DELAY_MICROSECONDS: u64 = 250_000;
/// Long past when the alarm should have gone off.
const ALARM_WAIT_MICROSECONDS: u64 = 1_000_000;

static SYSTEM_TIMER: AtomicUsize = AtomicUsize::new(0);
/// When the alarm's interrupt arrived, by the system timer, or 0 if it hasn't.
static FIRED_AT: AtomicU64 = AtomicU64::new(0);

fn system_timer() -> SystemTimer {
    SystemTimer::new(SYSTEM_TIMER.load(Ordering::Relaxed))
}

fn alarm() {
    let timer = system_timer();
    FIRED_AT.store(timer.now(), Ordering::Relaxed);
    timer.clear(ALARM_CHANNEL);
}

/// Compares the two counters, then sets an alarm and reports
/// how late its interrupt arrived.
pub fn check() {
    let channel = ALARM_CHANNEL.number();
    let (base, id, from) = match devices::system_timer(channel) {
        Some((base, id)) => (base, id, "the device tree"),
        None => (board::SYSTEM_TIMER, board::SYSTEM_TIMER_FIRST_INTERRUPT + channel as u32, "the board's defaults"),
    };
    SYSTEM_TIMER.store(base, Ordering::Relaxed);
    let timer = system_timer();
    let (arm, system) = (uptime_microseconds(), timer.now());
    println!("Uptime: {arm} us by the ARM counter, {system} us by the system timer.");

    if !interrupts::register(id, interrupts::NORMAL_PRIORITY, alarm) {
        println!("No room to register the alarm.");
        return;
    }
    FIRED_AT.store(0, Ordering::Relaxed);
    let target = timer.now() + ALARM_DELAY_MICROSECONDS;
    timer.set_alarm(ALARM_CHANNEL, target);
    while FIRED_AT.load(Ordering::Relaxed) == 0 && timer.now() < target + ALARM_WAIT_MICROSECONDS {}
    match FIRED_AT.load(Ordering::Relaxed) {
        0 => println!(
            "Alarm on system timer channel {channel}, interrupt {id} from {from}, never arrived; matched: {}.",
            timer.matched(ALARM_CHANNEL)
        ),
        fired => println!(
            "Alarm on system timer channel {channel}, interrupt {id} from {from}: arrived {} us after it was due.",
            fired.saturating_sub(target)
        ),
    }
}

/// How long to count cycles for, to measure the CPU's clock.
const CYCLE_SAMPLE_MICROSECONDS: u64 = 10_000;
const MICROSECONDS_PER_MEGAHERTZ_CYCLE: u64 = 1;

/// Starts the cycle counter, and measures the CPU's clock with it against the ARM
/// counter, which should give the rate the firmware set.
pub fn check_cycles() {
    crate::arch::cycles::start();
    let (start_time, start_cycles) = (uptime_microseconds(), crate::arch::cycles::now());
    while uptime_microseconds() - start_time < CYCLE_SAMPLE_MICROSECONDS {}
    let (end_time, end_cycles) = (uptime_microseconds(), crate::arch::cycles::now());
    let megahertz = (end_cycles - start_cycles) * MICROSECONDS_PER_MEGAHERTZ_CYCLE / (end_time - start_time);
    println!("Cycle counter: {} cycles in {} us, so the CPU runs at about {megahertz} MHz.", end_cycles - start_cycles, end_time - start_time);
}
