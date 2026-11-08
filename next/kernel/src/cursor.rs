//! The mouse cursor. There are no layers, so it saves the pixels it covers, and puts them
//! back before anything else draws.

use crate::image;
use crate::video::{Color, Framebuffer};

/// Made from `cursor.png` by `scripts/make-cursor.py`, in the format `image::draw` reads.
static SPRITE: &[u8] = include_bytes!("cursor.bin");
const WIDTH: usize = 32;
const HEIGHT: usize = 32;
/// Where the sprite's tip is, which is the point the position refers to.
const TIP_X: usize = 2;
const TIP_Y: usize = 2;

pub struct Cursor {
    /// The tip's position. It doesn't appear until the mouse first moves.
    x: usize,
    y: usize,
    moved: bool,
    /// The sprite's top-left corner, while it's on the screen.
    shown_at: Option<(usize, usize)>,
    saved: [Color; WIDTH * HEIGHT],
}

impl Cursor {
    pub fn new(framebuffer: &Framebuffer) -> Self {
        let (x, y) = (framebuffer.width / 2, framebuffer.height / 2);
        Self { x, y, moved: false, shown_at: None, saved: [0; WIDTH * HEIGHT] }
    }

    /// Moves the tip, keeping it on the screen. Takes effect at the next `show`.
    pub fn move_by(&mut self, framebuffer: &Framebuffer, dx: i32, dy: i32) {
        let clamp = |position: usize, delta: i32, size: usize| {
            (position as i32).saturating_add(delta).clamp(0, size as i32 - 1) as usize
        };
        self.x = clamp(self.x, dx, framebuffer.width);
        self.y = clamp(self.y, dy, framebuffer.height);
        self.moved = true;
    }

    /// Puts back what the cursor was covering. Call this before drawing anything.
    pub fn hide(&mut self, framebuffer: &Framebuffer) {
        if let Some((left, top)) = self.shown_at.take() {
            for (row, pixels) in self.saved.chunks_exact(WIDTH).enumerate() {
                framebuffer.put_row(left, top + row, pixels.iter().copied());
            }
        }
    }

    /// Saves what's under the cursor and draws it. Call this after drawing everything.
    pub fn show(&mut self, framebuffer: &Framebuffer) {
        if !self.moved {
            return;
        }
        let (left, top) = (self.x.saturating_sub(TIP_X), self.y.saturating_sub(TIP_Y));
        for (row, pixels) in self.saved.chunks_exact_mut(WIDTH).enumerate() {
            framebuffer.get_row(left, top + row, pixels);
        }
        if image::draw(framebuffer, left, top, SPRITE).is_ok() {
            self.shown_at = Some((left, top));
        }
    }
}

const _: () = assert!(WIDTH * HEIGHT > 0 && TIP_X < WIDTH && TIP_Y < HEIGHT);
