//! The watchdog: rebooting on request, and a countdown that reboots the board if the
//! kernel stops feeding it. Nothing leaves it counting, as it carries on while JTAG has
//! the core halted, and would reset the board in the middle of a debugging session.

use crate::board::{self, watchdog::{Watchdog, TICKS_PER_SECOND}};
use crate::devices;

/// The check's countdown, far shorter than it runs, so a missed feed would show.
const CHECK_COUNTDOWN_MILLISECONDS: u32 = 100;
const CHECK_FEED_EVERY_MILLISECONDS: u32 = 20;
const CHECK_MILLISECONDS: u32 = 300;
const MICROSECONDS_PER_MILLISECOND: u32 = 1000;
const MILLISECONDS_PER_SECOND: u32 = 1000;

/// The watchdog the device tree names, or the board's when there's no tree, as reboots
/// are essential.
pub fn find() -> Watchdog {
    Watchdog::new(devices::address_of("watchdog").unwrap_or(board::WATCHDOG))
}

fn ticks(milliseconds: u32) -> u32 {
    milliseconds * TICKS_PER_SECOND / MILLISECONDS_PER_SECOND
}

fn milliseconds(ticks: u32) -> u32 {
    ticks * MILLISECONDS_PER_SECOND / TICKS_PER_SECOND
}

/// Restarts the Pi, booting the usual kernel, or the trial one once.
pub fn reboot(trial: bool) -> ! {
    board::reboot(find(), trial)
}

/// Arms the watchdog with a short countdown, keeps feeding it for longer than that, and
/// cancels it. Reaching the end at all shows feeding works; the least left on the
/// countdown shows how close it came. It can't say whether the watchdog caused the last
/// reset: on 2026-11-07, `PM_RSTS` read 0 after both a reboot and a starved countdown, so
/// the firmware clears it first. Needs a device tree: QEMU's watchdog resets the moment it's armed, ignoring the
/// countdown, so it would reboot forever.
pub fn check() {
    let Some(base) = devices::address_of("watchdog") else {
        println!("Watchdog: not checked, as the device tree doesn't list it.");
        return;
    };
    let watchdog = Watchdog::new(base);
    let countdown = ticks(CHECK_COUNTDOWN_MILLISECONDS);
    let start = board::microseconds();
    let elapsed = || board::microseconds().wrapping_sub(start) / MICROSECONDS_PER_MILLISECOND;
    let mut least = countdown;
    let mut fed = 0;
    watchdog.arm(countdown);
    while elapsed() < CHECK_MILLISECONDS {
        let fed_at = elapsed();
        while elapsed().wrapping_sub(fed_at) < CHECK_FEED_EVERY_MILLISECONDS {}
        least = least.min(watchdog.remaining());
        watchdog.arm(countdown);
        fed += 1;
    }
    watchdog.cancel();
    println!(
        "Watchdog: fed {fed} times in {CHECK_MILLISECONDS} ms with a {CHECK_COUNTDOWN_MILLISECONDS} ms countdown, which got down to {} ms. Cancelled: {}.",
        milliseconds(least),
        !watchdog.armed()
    );
}
