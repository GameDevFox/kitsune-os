//! The core's own virtual timer, which counts down and raises an interrupt at zero.
//! EL1 can always use it, unlike the physical timer, which EL2 must allow.

use core::arch::asm;

/// CNTV_CTL_EL0: bit 0 enables the timer, and bit 1 would mask its interrupt.
const ENABLE: u64 = 1;

/// How many times the timer's counter ticks a second, as set by the firmware.
pub fn frequency() -> u64 {
    let value: u64;
    // SAFETY: reading CNTFRQ_EL0 has no side effects.
    unsafe { asm!("mrs {}, cntfrq_el0", out(reg) value, options(nomem, nostack)) };
    value
}

/// Raises the timer's interrupt after `ticks` of its counter, then again each time
/// `restart` is called. Starting also clears a pending interrupt.
pub fn start(ticks: u64) {
    restart(ticks);
    // SAFETY: only enables this core's virtual timer.
    unsafe { asm!("msr cntv_ctl_el0, {}", "isb", in(reg) ENABLE, options(nomem, nostack)) };
}

/// Sets the next interrupt `ticks` from now, which also clears the one now pending.
pub fn restart(ticks: u64) {
    // SAFETY: only sets this core's virtual timer.
    unsafe { asm!("msr cntv_tval_el0, {}", "isb", in(reg) ticks, options(nomem, nostack)) };
}

/// The counter the timer counts down against, which runs at `frequency`, from when the
/// core started. Its 64 bits last thousands of years at the Pi 4's 54 MHz.
pub fn counter() -> u64 {
    let value: u64;
    // SAFETY: reading CNTVCT_EL0 has no side effects. The barrier stops the read being
    // made early, out of order with the code before it.
    unsafe { asm!("isb", "mrs {}, cntvct_el0", out(reg) value, options(nomem, nostack)) };
    value
}
