//! Drawing into a framebuffer. Knows nothing about how a board provides one.

use core::ptr::{read_volatile, write_volatile};

/// 32 bits per pixel, as `0xAARRGGBB`.
pub type Color = u32;

pub const BLACK: Color = 0xFF00_0000;

const BYTES_PER_PIXEL: usize = 4;

pub struct Framebuffer {
    /// Must be the address of `pitch * height` bytes of writable memory.
    pub base: usize,
    pub width: usize,
    pub height: usize,
    /// Bytes per row, which can be more than `width * BYTES_PER_PIXEL`.
    pub pitch: usize,
}

impl Framebuffer {
    /// Draws a run of pixels along one row, ignoring anything off the screen.
    pub fn put_row(&self, x: usize, y: usize, colors: impl Iterator<Item = Color>) {
        if y >= self.height {
            return;
        }
        let row = self.base + y * self.pitch;
        for (column, color) in (x..self.width).zip(colors) {
            // SAFETY: `y` was checked above, and `zip` stops at the row's end.
            unsafe { write_volatile((row + column * BYTES_PER_PIXEL) as *mut u32, color) };
        }
    }

    /// Reads a run of pixels along one row. Anything off the screen stays as it was.
    pub fn get_row(&self, x: usize, y: usize, colors: &mut [Color]) {
        if y >= self.height {
            return;
        }
        let row = self.base + y * self.pitch;
        for (column, color) in (x..self.width).zip(colors) {
            // SAFETY: `y` was checked above, and `zip` stops at the row's end.
            *color = unsafe { read_volatile((row + column * BYTES_PER_PIXEL) as *const u32) };
        }
    }

    pub fn fill(&self, color: Color) {
        self.fill_rect(0, 0, self.width, self.height, color);
    }

    /// Fills a rectangle, clipped to the screen. Volatile, because the display
    /// hardware reads this memory and the compiler doesn't know that.
    pub fn fill_rect(&self, x: usize, y: usize, width: usize, height: usize, color: Color) {
        let right = x.saturating_add(width).min(self.width);
        let bottom = y.saturating_add(height).min(self.height);

        for row in y..bottom {
            for column in x..right {
                let address = self.base + row * self.pitch + column * BYTES_PER_PIXEL;
                // SAFETY: the bounds above keep this inside the framebuffer.
                unsafe { write_volatile(address as *mut u32, color) };
            }
        }
    }
}
