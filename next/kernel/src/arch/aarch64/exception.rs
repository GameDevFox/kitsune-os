//! Exception handling at EL1. `vectors.s` saves the CPU state and calls here.

use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering};

use super::halt;

const FRAME_SIZE: usize = 272;

/// The state saved on entry and restored on return. Must match `vectors.s`.
#[repr(C)]
pub struct TrapFrame {
    x: [u64; 31],
    elr: u64,
    spsr: u64,
    /// Keeps the frame a multiple of 16 bytes, as the stack pointer requires.
    _padding: u64,
}

const _: () = assert!(core::mem::size_of::<TrapFrame>() == FRAME_SIZE);

const KINDS_PER_SOURCE: u64 = 4;
const SOURCES: [&str; 4] = ["current EL, SP0", "current EL, SPx", "lower EL, AArch64", "lower EL, AArch32"];
const TYPES: [&str; 4] = ["synchronous", "IRQ", "FIQ", "SError"];
const TYPE_SYNCHRONOUS: u64 = 0;
/// An IRQ while the kernel itself was running, using its own stack pointer.
const KIND_IRQ_FROM_KERNEL: u64 = 5;

/// What `set_interrupt_handler` gave, as an address, or 0 for none.
static INTERRUPT_HANDLER: AtomicUsize = AtomicUsize::new(0);

/// Has `handler` called for each IRQ. It must find out what interrupted, and finish
/// with it, or the same one will arrive again straight away.
pub fn set_interrupt_handler(handler: fn()) {
    INTERRUPT_HANDLER.store(handler as usize, Ordering::Release);
}

// ESR_EL1: why the exception happened.
const ESR_CLASS_SHIFT: u64 = 26;
const ESR_CLASS_MASK: u64 = 0x3F;
const ESR_FAULT_STATUS_MASK: u64 = 0x3F;
const ESR_BREAKPOINT_NUMBER_MASK: u64 = 0xFFFF;

const CLASS_INSTRUCTION_ABORT: u64 = 0x21;
const CLASS_DATA_ABORT: u64 = 0x25;
const CLASS_BREAKPOINT: u64 = 0x3C;

const FAULT_ALIGNMENT: u64 = 0x21;

const INSTRUCTION_SIZE: u64 = 4;
const REGISTERS_PER_ROW: usize = 3;

/// Called from `vectors.s`. Returning resumes at `frame.elr`.
#[no_mangle]
extern "C" fn handle_exception(kind: u64, frame: &mut TrapFrame) {
    if kind == KIND_IRQ_FROM_KERNEL {
        let address = INTERRUPT_HANDLER.load(Ordering::Acquire);
        if address != 0 {
            // SAFETY: only `set_interrupt_handler` stores here, and it stores a `fn()`.
            let handler: fn() = unsafe { core::mem::transmute(address) };
            handler();
            return;
        }
    }

    let esr = read_esr();
    let class = (esr >> ESR_CLASS_SHIFT) & ESR_CLASS_MASK;

    // A breakpoint is the one exception that is always safe to resume: it did
    // nothing except stop here. ELR points at the BRK, so step over it.
    if kind % KINDS_PER_SOURCE == TYPE_SYNCHRONOUS && class == CLASS_BREAKPOINT {
        let number = esr & ESR_BREAKPOINT_NUMBER_MASK;
        println!("Breakpoint #{number} at {:#x}, resuming", frame.elr);
        frame.elr += INSTRUCTION_SIZE;
        return;
    }

    report(kind, class, esr, frame);
    halt()
}

fn report(kind: u64, class: u64, esr: u64, frame: &TrapFrame) {
    let source = SOURCES[(kind / KINDS_PER_SOURCE) as usize];
    let kind_type = TYPES[(kind % KINDS_PER_SOURCE) as usize];

    println!("FATAL EXCEPTION: {} ({kind_type}, {source})", describe(class, esr));
    print!("  at {:#x}", frame.elr);
    if class == CLASS_DATA_ABORT || class == CLASS_INSTRUCTION_ABORT {
        print!(" accessing {:#x}", read_far());
    }
    println!(" ESR {esr:#x} SPSR {:#x}", frame.spsr);
    // Compact, so that the report fits the on-screen text area without wrapping.
    for (row, registers) in frame.x.chunks(REGISTERS_PER_ROW).enumerate() {
        for (column, value) in registers.iter().enumerate() {
            print!("  x{:<2} {value:#018x}", row * REGISTERS_PER_ROW + column);
        }
        println!();
    }
}

fn describe(class: u64, esr: u64) -> &'static str {
    let status = esr & ESR_FAULT_STATUS_MASK;
    match class {
        CLASS_DATA_ABORT if status == FAULT_ALIGNMENT => "data abort: alignment fault",
        CLASS_DATA_ABORT => "data abort",
        CLASS_INSTRUCTION_ABORT if status == FAULT_ALIGNMENT => "instruction abort: alignment fault",
        CLASS_INSTRUCTION_ABORT => "instruction abort",
        CLASS_BREAKPOINT => "breakpoint",
        _ => "unrecognized exception",
    }
}

fn read_esr() -> u64 {
    let value: u64;
    // SAFETY: reading ESR_EL1 at EL1 has no side effects.
    unsafe { asm!("mrs {}, esr_el1", out(reg) value, options(nomem, nostack)) };
    value
}

fn read_far() -> u64 {
    let value: u64;
    // SAFETY: reading FAR_EL1 at EL1 has no side effects.
    unsafe { asm!("mrs {}, far_el1", out(reg) value, options(nomem, nostack)) };
    value
}
