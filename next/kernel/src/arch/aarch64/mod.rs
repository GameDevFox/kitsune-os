use core::arch::{asm, global_asm};

pub mod cycles;
mod exception;
mod mmu;
pub mod timer;

pub use exception::set_interrupt_handler;
pub use mmu::enable as enable_mmu;

global_asm!(include_str!("boot.s"));
global_asm!(include_str!("vectors.s"));

pub fn halt() -> ! {
    loop {
        // SAFETY: wfe only pauses this core until the next event.
        unsafe { asm!("wfe", options(nomem, nostack)) };
    }
}

/// Lets IRQs in. Set the handler first, with `set_interrupt_handler`.
pub fn enable_interrupts() {
    // SAFETY: clears only the IRQ mask bit; the handler is set before this is called.
    unsafe { asm!("msr daifclr, #{}", const IRQ_MASK_BIT, options(nomem, nostack)) };
}

/// The IRQ bit, in the immediate `daifclr` and `daifset` take: debug, SError, IRQ, FIQ.
const IRQ_MASK_BIT: u32 = 0b0010;

/// Raises a breakpoint exception, which the handler resumes from.
pub fn trigger_breakpoint() {
    // SAFETY: the exception handler steps over the BRK and restores all registers.
    unsafe { asm!("brk #0") };
}
