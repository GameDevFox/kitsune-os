//! The drivers' proofs of concept, run on request rather than at boot, as together they
//! take about 1.6 s, and quick reloads matter more. Each prints its own result.

use crate::{arch, interrupts, random, temperature, time, watchdog};

/// Runs them all, from the page or `POST /key`.
pub const KEY: u8 = b'c';

/// Needs interrupts started and the CPU clock raised, as at boot, for the cycle count to
/// mean anything.
pub fn run_all() {
    println!("Running the checks.");
    // Exercises the exception path; remove once something else depends on it.
    arch::trigger_breakpoint();
    println!("Still running after a breakpoint.");
    interrupts::check_rate();
    time::check();
    time::check_cycles();
    temperature::check();
    random::check();
    watchdog::check();
    println!("Checks done.");
}
