//! The main loop: a square circling the logo, a cube in the logo's place on request,
//! the cursor, and keys, between frames.

use crate::assets::Assets;
use crate::board;
use crate::checks;
use crate::console;
use crate::cube::Cube;
use crate::cursor::Cursor;
use crate::devices;
use crate::gpu_cube::GpuCube;
use crate::pictures::{self, CLEAR_KEY};
use crate::sound;
use crate::spin::Spinner;
use crate::square::Square;
use crate::upload;
use crate::video::Framebuffer;

pub const FRAMES_PER_SECOND: u32 = 60;
const MICROSECONDS_PER_SECOND: u32 = 1_000_000;
/// Rounded to the nearest microsecond.
const FRAME_MICROSECONDS: u32 = (MICROSECONDS_PER_SECOND + FRAMES_PER_SECOND / 2) / FRAMES_PER_SECOND;

/// Turn a spinning cube on and off, in place of the logo, drawn by the CPU or the GPU.
const CUBE_KEY: u8 = b'5';
const GPU_CUBE_KEY: u8 = b'6';

/// Keeps handling serial input forever, for when there's no screen to do anything else
/// with. Commands that only draw are dropped.
pub fn run_without_screen() -> ! {
    loop {
        console::echo_pending_input();
        upload::give_up_if_stalled();
        let _ = console::take_mouse();
        if console::take_latest_key() == Some(checks::KEY) {
            checks::run_all();
        }
    }
}

/// Runs the animation forever, handling serial input between frames, and acting on the
/// newest key. The logo spins if `logo_drawn`, until a clear. The pictures that keys
/// draw, and the voice, come from `assets`.
pub fn run(framebuffer: &Framebuffer, logo_drawn: bool, assets: &mut Option<Assets>) -> ! {
    let mut square = Square::new();
    let mut spinner = logo_drawn.then(|| Spinner::capture(framebuffer));
    let mut frame_start = board::microseconds();
    let mut cursor = Cursor::new(framebuffer);
    let mut cube = Cube::new();
    let mut cube_on = false;
    let mut gpu_cube: Option<GpuCube> = None;
    let mut player = sound::Player::new();

    loop {
        // Nothing but serial gets attention during an upload, so that no bytes get lost.
        while upload::is_active() {
            console::echo_pending_input();
            upload::give_up_if_stalled();
            frame_start = board::microseconds();
        }

        // Everything below draws while the cursor is off the screen, so it never ends up
        // in what the cursor saved.
        cursor.hide(framebuffer);
        let (dx, dy) = console::take_mouse();
        if dx != 0 || dy != 0 {
            cursor.move_by(framebuffer, dx, dy);
        }

        square.hide(framebuffer);
        // A cube takes the logo's place while either is on.
        if let Some(spinner) = spinner.as_mut() {
            if cube_on || gpu_cube.is_some() {
                spinner.hide(framebuffer);
            } else {
                spinner.show(framebuffer);
            }
            spinner.step_and_draw(framebuffer);
        }
        if let Some(running) = gpu_cube.as_mut() {
            if !running.draw() {
                running.stop(framebuffer);
                gpu_cube = None;
            }
        } else if cube_on {
            cube.draw(framebuffer);
        }
        // It circles the logo's place, outside where a cube draws, so it carries on.
        square.step_and_show(framebuffer);
        cursor.show(framebuffer);
        player.poll();

        // The timer's count wraps around, which wrapping subtraction handles.
        while board::microseconds().wrapping_sub(frame_start) < FRAME_MICROSECONDS {
            console::echo_pending_input();
        }
        frame_start = frame_start.wrapping_add(FRAME_MICROSECONDS);

        if let Some(key) = console::take_latest_key() {
            // Whatever the key draws must not end up in what these saved.
            cursor.hide(framebuffer);
            square.hide(framebuffer);
            // It would bring the logo straight back.
            if key == CLEAR_KEY {
                spinner = None;
            }
            if key == CUBE_KEY || key == GPU_CUBE_KEY {
                // Either key stops whichever cube is running, before maybe starting its own.
                let gpu_was_on = gpu_cube.is_some();
                let cpu_was_on = cube_on;
                if let Some(mut running) = gpu_cube.take() {
                    running.stop(framebuffer);
                }
                if cube_on {
                    cube.stop(framebuffer);
                    cube_on = false;
                }
                if key == CUBE_KEY && !cpu_was_on {
                    cube_on = true;
                }
                if key == GPU_CUBE_KEY && !gpu_was_on {
                    gpu_cube = GpuCube::new(framebuffer);
                }
            } else if key == devices::PRINT_KEY {
                devices::print_all();
            } else if key == checks::KEY {
                checks::run_all();
            } else if !player.handle_key(key, assets) {
                pictures::handle_key(framebuffer, assets, key);
            }
            // That can take a while, and the square shouldn't rush to catch up.
            frame_start = board::microseconds();
        }
    }
}
