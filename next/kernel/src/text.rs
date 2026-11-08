//! Text on the framebuffer, using a fixed-size bitmap font.

use crate::video::{Color, Framebuffer, BLACK};

const GLYPH_WIDTH: usize = 16;
const GLYPH_HEIGHT: usize = 30;
const BYTES_PER_GLYPH_ROW: usize = 2;
const GLYPH_COUNT: usize = 128;
const BYTES_PER_GLYPH: usize = GLYPH_HEIGHT * BYTES_PER_GLYPH_ROW;

/// From the old project's font sheet, packed by `scripts/make-font.py`.
static FONT: &[u8; GLYPH_COUNT * BYTES_PER_GLYPH] = include_bytes!("font-16x30.bin");

const TEXT_COLOR: Color = 0xFFE0_E0E0;
/// Stands in for anything the font can't show.
const UNPRINTABLE: u8 = b'.';
const FIRST_PRINTABLE: u8 = 0x20;
const LAST_PRINTABLE: u8 = 0x7E;

/// The left of the screen, top to bottom, is for text. The logo and the square circling
/// it sit just to the right, and Aki at the far right.
const TEXT_COLUMNS: usize = 64;
const TEXT_ROWS: usize = 36;
/// How far the text reaches across the screen, which nothing else may draw inside.
pub const TEXT_WIDTH: usize = TEXT_COLUMNS * GLYPH_WIDTH;

pub struct TextScreen {
    framebuffer: Framebuffer,
    columns: usize,
    rows: usize,
    column: usize,
    row: usize,
    /// Scanlines of the current row still to be cleared.
    lines_to_clear: usize,
    /// The last character filled its row, so a newline now has nothing to do.
    just_wrapped: bool,
}

impl TextScreen {
    pub fn new(framebuffer: Framebuffer) -> Self {
        let columns = TEXT_COLUMNS.min(framebuffer.width / GLYPH_WIDTH);
        let rows = TEXT_ROWS.min(framebuffer.height / GLYPH_HEIGHT).max(1);
        Self { framebuffer, columns, rows, column: 0, row: 0, lines_to_clear: 0, just_wrapped: false }
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            while self.clear_step() {}
            match byte {
                b'\n' if self.just_wrapped => self.just_wrapped = false,
                b'\n' => self.new_line(),
                b'\r' => self.column = 0,
                FIRST_PRINTABLE..=LAST_PRINTABLE => self.put(byte),
                _ => self.put(UNPRINTABLE),
            }
        }
    }

    fn put(&mut self, byte: u8) {
        let start = usize::from(byte) * BYTES_PER_GLYPH;
        let glyph = &FONT[start..start + BYTES_PER_GLYPH];
        let left = self.column * GLYPH_WIDTH;
        let top = self.row * GLYPH_HEIGHT;

        for (y, row) in glyph.chunks_exact(BYTES_PER_GLYPH_ROW).enumerate() {
            let bits = u16::from_be_bytes([row[0], row[1]]);
            let colors = (0..GLYPH_WIDTH).map(|x| {
                let lit = bits & (1 << (GLYPH_WIDTH - 1 - x)) != 0;
                if lit { TEXT_COLOR } else { BLACK }
            });
            self.framebuffer.put_row(left, top + y, colors);
        }
        self.column += 1;
        self.just_wrapped = false;
        // Wrap now, so the new row gets cleared before the next character arrives.
        if self.column == self.columns {
            self.new_line();
            self.just_wrapped = true;
        }
    }

    /// Clears one scanline of a new row, returning false once none are left.
    /// Clearing a whole row at once would take long enough to lose serial input,
    /// so a caller watching the UART can check it between steps.
    pub fn clear_step(&mut self) -> bool {
        if self.lines_to_clear == 0 {
            return false;
        }
        self.lines_to_clear -= 1;
        let y = self.row * GLYPH_HEIGHT + self.lines_to_clear;
        self.framebuffer.fill_rect(0, y, self.columns * GLYPH_WIDTH, 1, BLACK);
        true
    }

    /// Wraps back to the top rather than scrolling, as scrolling would copy most
    /// of the screen, which is slow with the MMU off.
    fn new_line(&mut self) {
        while self.clear_step() {}
        self.column = 0;
        self.row = (self.row + 1) % self.rows;
        self.lines_to_clear = GLYPH_HEIGHT;
    }
}
