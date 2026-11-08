//! A square circling the logo, cycling through the rainbow's colors. It passes over
//! Aki's hand and hair, so like the cursor it saves the pixels it covers and puts them
//! back before moving, rather than painting black behind itself.

use crate::animation::FRAMES_PER_SECOND;
use crate::pictures::{LOGO_CENTER_X, LOGO_CENTER_Y};
use crate::rainbow;
use crate::text::TEXT_WIDTH;
use crate::video::{Color, Framebuffer};

const SIZE: usize = 40;
const HALF: usize = SIZE / 2;
/// Clear of the logo's corners, 153 px from its center, with the square's half diagonal.
const RADIUS: usize = 190;
/// The colors go all the way round once a second, so each frame skips some.
const RAINBOW_FRAMES: usize = FRAMES_PER_SECOND as usize;

/// Positions keep this many fractional bits, as a step moves less than a pixel.
const FRACTION_BITS: u32 = 16;
/// Each step turns by `TURN_NUMERATOR / 2^TURN_SHIFT` of a radian: 3/128, so a lap
/// takes about 268 steps, 4.5 s at 60 frames a second.
const TURN_NUMERATOR: i64 = 3;
const TURN_SHIFT: u32 = 7;

// Never over the text, which would draw under it and be erased when it moved, nor off
// the top of the screen.
const _: () = assert!(LOGO_CENTER_X - RADIUS - HALF >= TEXT_WIDTH);
const _: () = assert!(LOGO_CENTER_Y >= RADIUS + HALF);

/// The Minsky circle algorithm. Using the new `x` to update `y` is what keeps the
/// orbit closed; using the old one would make it spiral outward.
struct Orbit {
    x: i64,
    y: i64,
}

impl Orbit {
    fn new(radius: i64) -> Self {
        Self { x: radius << FRACTION_BITS, y: 0 }
    }

    fn step(&mut self) {
        self.x -= (self.y * TURN_NUMERATOR) >> TURN_SHIFT;
        self.y += (self.x * TURN_NUMERATOR) >> TURN_SHIFT;
    }

    fn position(&self) -> (i64, i64) {
        (self.x >> FRACTION_BITS, self.y >> FRACTION_BITS)
    }
}

pub struct Square {
    orbit: Orbit,
    /// Frames into the rainbow's cycle, which picks its color.
    rainbow_frame: usize,
    /// The top-left corner, while it's on the screen.
    shown_at: Option<(usize, usize)>,
    saved: [Color; SIZE * SIZE],
}

impl Square {
    pub fn new() -> Self {
        Self { orbit: Orbit::new(RADIUS as i64), rainbow_frame: 0, shown_at: None, saved: [0; SIZE * SIZE] }
    }

    /// Puts back what the square was covering. Call this before drawing anything there.
    pub fn hide(&mut self, framebuffer: &Framebuffer) {
        if let Some((left, top)) = self.shown_at.take() {
            for (row, pixels) in self.saved.chunks_exact(SIZE).enumerate() {
                framebuffer.put_row(left, top + row, pixels.iter().copied());
            }
        }
    }

    /// Moves one step round and on a color, saves what's under the square there, and
    /// draws it.
    pub fn step_and_show(&mut self, framebuffer: &Framebuffer) {
        self.orbit.step();
        self.rainbow_frame = (self.rainbow_frame + 1) % RAINBOW_FRAMES;
        let (x, y) = self.orbit.position();
        let left = (LOGO_CENTER_X as i64 + x) as usize - HALF;
        let top = (LOGO_CENTER_Y as i64 + y) as usize - HALF;
        for (row, pixels) in self.saved.chunks_exact_mut(SIZE).enumerate() {
            framebuffer.get_row(left, top + row, pixels);
        }
        framebuffer.fill_rect(left, top, SIZE, SIZE, rainbow::COLORS[self.rainbow_frame * rainbow::COUNT / RAINBOW_FRAMES]);
        self.shown_at = Some((left, top));
    }
}
