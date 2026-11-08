//! Interrupts: the GIC, and which handler each interrupt number goes to. Drivers
//! register their own, so this knows nothing about any of them. The core's timer ticks
//! through here too, counted to show that interrupts arrive.

use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use crate::arch::{self, timer};
use crate::board;
use crate::devices;
use crate::gic::{Gic, SPURIOUS};

pub const TICKS_PER_SECOND: u64 = 100;
/// Lower is more urgent, and only the top bits count in the non-secure view.
pub const NORMAL_PRIORITY: u8 = 0xA0;
/// The virtual timer's interrupt on ARM's standard layout, when there's no device tree.
const USUAL_VIRTUAL_TIMER_INTERRUPT: u32 = 27;
const MICROSECONDS_PER_SECOND: u32 = 1_000_000;

/// How many interrupts can have handlers at once.
const MOST_HANDLERS: usize = 8;
/// Marks a free slot: the GIC never hands this out, as it means "nothing pending".
const FREE: u32 = SPURIOUS;

static DISTRIBUTOR: AtomicUsize = AtomicUsize::new(0);
static CPU_INTERFACE: AtomicUsize = AtomicUsize::new(0);
// Each slot's interrupt number, and its handler as an address. A slot's handler is
// stored before its number, so the dispatcher never finds a number without a handler.
static IDS: [AtomicU32; MOST_HANDLERS] = [const { AtomicU32::new(FREE) }; MOST_HANDLERS];
static HANDLERS: [AtomicUsize; MOST_HANDLERS] = [const { AtomicUsize::new(0) }; MOST_HANDLERS];

/// The timer's counts between ticks.
static TICK_LENGTH: AtomicUsize = AtomicUsize::new(0);
static TICKS: AtomicU32 = AtomicU32::new(0);
/// Interrupts that arrived with no handler, and were disabled so they couldn't repeat.
static UNHANDLED: AtomicU32 = AtomicU32::new(0);

fn gic() -> Gic {
    Gic::new(DISTRIBUTOR.load(Ordering::Relaxed), CPU_INTERFACE.load(Ordering::Relaxed))
}

/// Finds the GIC, from the device tree if there is one, starts it, and lets interrupts
/// in. None arrive until something is registered.
pub fn start() {
    let (distributor, cpu_interface, from) = match devices::interrupt_controller() {
        Some((distributor, cpu_interface)) => (distributor, cpu_interface, "the device tree"),
        None => (board::GIC_DISTRIBUTOR, board::GIC_CPU_INTERFACE, "the board's defaults"),
    };
    DISTRIBUTOR.store(distributor, Ordering::Relaxed);
    CPU_INTERFACE.store(cpu_interface, Ordering::Relaxed);
    gic().start();
    arch::set_interrupt_handler(dispatch);
    arch::enable_interrupts();
    println!("Interrupts on: GIC at {distributor:#x}, from {from}.");
}

/// Has `handler` called whenever interrupt `id` arrives, and lets it through. The handler
/// must make its device stop asking, or it will be called again at once. Returns false
/// if every slot is taken.
pub fn register(id: u32, priority: u8, handler: fn()) -> bool {
    let Some(slot) = IDS.iter().position(|slot| slot.load(Ordering::Acquire) == FREE) else {
        return false;
    };
    HANDLERS[slot].store(handler as usize, Ordering::Release);
    IDS[slot].store(id, Ordering::Release);
    gic().enable(id, priority);
    true
}

/// Called for every IRQ, with IRQs masked.
fn dispatch() {
    let gic = gic();
    let interrupt = gic.acknowledge();
    if interrupt.id() == SPURIOUS {
        return;
    }
    match IDS.iter().position(|slot| slot.load(Ordering::Acquire) == interrupt.id()) {
        Some(slot) => {
            let address = HANDLERS[slot].load(Ordering::Acquire);
            // SAFETY: only `register` stores here, and it stores a `fn()`.
            let handler: fn() = unsafe { core::mem::transmute(address) };
            handler();
        }
        // Nothing would make its device stop asking, so it would arrive again forever.
        None => {
            gic.disable(interrupt.id());
            UNHANDLED.fetch_add(1, Ordering::Relaxed);
        }
    }
    gic.end(interrupt);
}

/// Starts the core's timer ticking, `TICKS_PER_SECOND` times a second.
pub fn start_tick() {
    let (id, from) = match devices::virtual_timer_interrupt() {
        Some(id) => (id, "the device tree"),
        None => (USUAL_VIRTUAL_TIMER_INTERRUPT, "ARM's usual layout"),
    };
    let frequency = timer::frequency();
    if frequency < TICKS_PER_SECOND {
        println!("The timer's frequency is {frequency} Hz, too slow to tick with.");
        return;
    }
    let tick_length = frequency / TICKS_PER_SECOND;
    TICK_LENGTH.store(tick_length as usize, Ordering::Relaxed);
    if !register(id, NORMAL_PRIORITY, tick) {
        println!("No room to register the timer tick.");
        return;
    }
    timer::start(tick_length);
    println!("Timer tick: interrupt {id}, from {from}; counter at {frequency} Hz, {TICKS_PER_SECOND} ticks a second.");
}

fn tick() {
    timer::restart(TICK_LENGTH.load(Ordering::Relaxed) as u64);
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// How many interrupts arrived with no handler, and were turned off.
pub fn unhandled() -> u32 {
    UNHANDLED.load(Ordering::Relaxed)
}

pub fn ticks() -> u32 {
    TICKS.load(Ordering::Relaxed)
}

/// Counts the ticks over one second and prints them, to show they arrive at the rate set.
pub fn check_rate() {
    let before = ticks();
    let start = board::microseconds();
    while board::microseconds().wrapping_sub(start) < MICROSECONDS_PER_SECOND {}
    println!(
        "Timer interrupts: {} in 1 s, expecting {TICKS_PER_SECOND}. {} arrived with no handler.",
        ticks().wrapping_sub(before),
        unhandled()
    );
}
