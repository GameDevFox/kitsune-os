//! Blinks the green activity LED from its own alarm, to show the kernel is alive without
//! a screen or serial. It owns system timer channel 3, and uses nothing but that and the
//! GPIO.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use crate::board::{self, gpio::Gpio, system_timer::{SystemTimer, CHANNEL_3}};
use crate::devices;
use crate::interrupts;

const HALF_PERIOD_MICROSECONDS: u64 = 500_000;

static GPIO: AtomicUsize = AtomicUsize::new(0);
static SYSTEM_TIMER: AtomicUsize = AtomicUsize::new(0);
static PIN: AtomicU32 = AtomicU32::new(0);
static ACTIVE_LOW: AtomicBool = AtomicBool::new(false);
static LIT: AtomicBool = AtomicBool::new(false);

fn gpio() -> Gpio {
    Gpio::new(GPIO.load(Ordering::Relaxed))
}

fn system_timer() -> SystemTimer {
    SystemTimer::new(SYSTEM_TIMER.load(Ordering::Relaxed))
}

fn light(on: bool) {
    gpio().set(PIN.load(Ordering::Relaxed), on != ACTIVE_LOW.load(Ordering::Relaxed));
    LIT.store(on, Ordering::Relaxed);
}

fn beat() {
    let timer = system_timer();
    timer.set_alarm(CHANNEL_3, timer.now() + HALF_PERIOD_MICROSECONDS);
    light(!LIT.load(Ordering::Relaxed));
}

/// Starts the LED blinking, once a second.
pub fn start() {
    let channel = CHANNEL_3.number();
    let (timer, id) = devices::system_timer(channel)
        .unwrap_or((board::SYSTEM_TIMER, board::SYSTEM_TIMER_FIRST_INTERRUPT + channel as u32));
    // An optional device, so there's no fallback without a device tree.
    let (Some((pin, active_low)), Some(gpio_base)) = (devices::activity_led(), devices::address_of("gpio")) else {
        println!("Heartbeat: no activity LED in the device tree, so skipped.");
        return;
    };
    GPIO.store(gpio_base, Ordering::Relaxed);
    SYSTEM_TIMER.store(timer, Ordering::Relaxed);
    PIN.store(pin, Ordering::Relaxed);
    ACTIVE_LOW.store(active_low, Ordering::Relaxed);

    gpio().make_output(pin);
    if !interrupts::register(id, interrupts::NORMAL_PRIORITY, beat) {
        println!("Heartbeat: no room to register its alarm.");
        return;
    }
    let timer = system_timer();
    timer.set_alarm(CHANNEL_3, timer.now() + HALF_PERIOD_MICROSECONDS);
    println!("Heartbeat: blinking the activity LED on GPIO {pin}, from system timer channel {channel}.");
}
