//! The core's cycle counter, in its performance monitors. It counts CPU clock cycles,
//! so it's far finer than any timer, and shows the clock's real speed.

use core::arch::asm;

/// PMCR_EL0: bit 0 enables the counters, and bit 2 resets the cycle counter.
const PMCR_ENABLE: u64 = 1 << 0;
const PMCR_RESET_CYCLES: u64 = 1 << 2;
/// PMCNTENSET_EL0's bit for the cycle counter.
const CYCLE_COUNTER: u64 = 1 << 31;

/// Starts the cycle counter from 0, counting at every exception level.
pub fn start() {
    // SAFETY: only this core's performance monitors change. PMCCFILTR_EL0 at 0 counts
    // cycles at EL1, where the kernel runs.
    unsafe {
        asm!(
            "msr pmccfiltr_el0, xzr",
            "mrs {value}, pmcr_el0",
            "orr {value}, {value}, {bits}",
            "msr pmcr_el0, {value}",
            "msr pmcntenset_el0, {cycles}",
            "isb",
            value = out(reg) _,
            bits = in(reg) PMCR_ENABLE | PMCR_RESET_CYCLES,
            cycles = in(reg) CYCLE_COUNTER,
            options(nomem, nostack),
        );
    }
}

/// Cycles since `start`.
pub fn now() -> u64 {
    let value: u64;
    // SAFETY: reading PMCCNTR_EL0 has no side effects. The barrier keeps the read in
    // order with the code around it.
    unsafe { asm!("isb", "mrs {}, pmccntr_el0", out(reg) value, options(nomem, nostack)) };
    value
}
