#![no_std]
#![no_main]

#[macro_use]
mod console;

mod animation;
mod assets;
mod card;
mod checks;
mod cube;
mod cursor;
mod devices;
mod gic;
mod gpu_cube;
mod heartbeat;
mod image;
mod interrupts;
mod pictures;
mod rainbow;
mod random;
mod sound;
mod spin;
mod square;
mod temperature;
mod text;
mod time;
mod trig;
mod upload;
mod video;
mod watchdog;

#[path = "arch/aarch64/mod.rs"]
mod arch;
#[path = "board/raspi4/mod.rs"]
mod board;

const SCREEN_WIDTH: u32 = 1920;
const SCREEN_HEIGHT: u32 = 1080;

const MICROSECONDS_PER_MILLISECOND: u32 = 1000;

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    // The timer restarts at a reset, so on a real boot this is how long the firmware took.
    // After `cargo pi`, it's just how long the board has been up.
    let started = board::microseconds();
    arch::enable_mmu(&board::MEMORY_MAP);
    board::console_init();
    println!("Hello, World!");
    println!("Kernel started {} ms after reset.", started / MICROSECONDS_PER_MILLISECOND);

    devices::report();

    match board::raise_cpu_clock() {
        Some(megahertz) => println!("CPU clock set to {megahertz} MHz."),
        None => println!("Couldn't raise the CPU clock."),
    }

    // The drivers' checks wait for `checks::KEY`, to keep boots quick.
    interrupts::start();
    interrupts::start_tick();
    heartbeat::start();

    // This happens with nothing attached to the HDMI port, and shouldn't stop anything
    // that doesn't need a screen, such as updating the kernel over serial.
    let Some(framebuffer) = board::framebuffer(SCREEN_WIDTH, SCREEN_HEIGHT) else {
        println!("The firmware gave no framebuffer, so carrying on without a screen.");
        card::report();
        report_ready(started);
        animation::run_without_screen();
    };
    println!(
        "Framebuffer at {:#x}, {}x{}, {} bytes per row.",
        framebuffer.base, framebuffer.width, framebuffer.height, framebuffer.pitch
    );
    framebuffer.fill(video::BLACK);

    // A copy for the square. It and the text draw to separate parts of the screen.
    let animation_framebuffer = video::Framebuffer { ..framebuffer };
    console::attach_screen(framebuffer);

    println!("Kitsune OS");
    card::report();
    let mut assets = assets::Assets::find(board::assets_region(), card::image_area());
    let logo_drawn = assets.is_some();
    match assets.as_mut() {
        Some(images) => {
            println!("Images are {}.", if images.on_card() { "on the SD card" } else { "in memory" });
            pictures::draw_startup(&animation_framebuffer, images);
        }
        None => println!("No images, in memory or on the SD card."),
    }
    println!("Type something and it will echo back.");
    println!("Keys: 1 draws the original Aki, 2, 3, 4 her at x1, x2, x4. 5 spins a cube, 6 with the GPU. 7 plays a tone, 8 a loaded clip. 9 prints the device tree over serial. 0 clears. c runs the checks.");
    report_ready(started);
    animation::run(&animation_framebuffer, logo_drawn, &mut assets)
}

/// How long the kernel took to reach its main loop, from `started`, a reading of
/// `board::microseconds` on entry, to keep an eye on reload times.
fn report_ready(started: u32) {
    let taken = board::microseconds().wrapping_sub(started) / MICROSECONDS_PER_MILLISECOND;
    println!("Ready {taken} ms after the kernel started.");
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{info}");
    arch::halt()
}
