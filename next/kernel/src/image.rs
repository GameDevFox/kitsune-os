//! Draws images stored in the format of `scripts/make-assets.py`: run-length
//! packets of symbols, a symbol being a full-color pixel or a palette index.

use core::iter;

use crate::video::{Color, Framebuffer};

const HEADER_BYTES: usize = 7;
const INDEX_SIZE_AT: usize = 4;
const PALETTE_COUNT_AT: usize = 5;
const PALETTE_ENTRY_BYTES: usize = 3;

const ESCAPE: u8 = 0xFF;
const MAX_RAW: usize = 256;
const FULL_COLOR: usize = 0;
const PIXEL_BYTES: usize = 4;
const FULLY_OPAQUE: u8 = 0xFF;
const OPAQUE_BITS: Color = 0xFF00_0000;

/// The image data ended early or made no sense.
#[derive(Debug)]
pub struct BadImage;

fn rgb(red: u8, green: u8, blue: u8) -> Color {
    OPAQUE_BITS | u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue)
}

/// How to turn this image's symbols into colors.
struct Symbols<'a> {
    /// 0 for full-color pixels, otherwise the size of a palette index.
    index_size: usize,
    palette: &'a [u8],
}

impl Symbols<'_> {
    fn size(&self) -> usize {
        if self.index_size == FULL_COLOR { PIXEL_BYTES } else { self.index_size }
    }

    /// Returns the color to draw, or `None` to skip the pixel. A full-color pixel
    /// is skipped unless fully solid, as there's no blending yet, so soft edges look
    /// jagged. Index 0 is always skip, and index N is palette entry N - 1.
    fn color(&self, symbol: &[u8]) -> Result<Option<Color>, BadImage> {
        if self.index_size == FULL_COLOR {
            let solid = symbol[3] == FULLY_OPAQUE;
            return Ok(solid.then(|| rgb(symbol[0], symbol[1], symbol[2])));
        }

        let index = symbol.iter().rev().fold(0, |value, &byte| value << 8 | usize::from(byte));
        if index == 0 {
            return Ok(None);
        }
        let start = (index - 1) * PALETTE_ENTRY_BYTES;
        let entry = self.palette.get(start..start + PALETTE_ENTRY_BYTES).ok_or(BadImage)?;
        Ok(Some(rgb(entry[0], entry[1], entry[2])))
    }
}

/// Tracks where the next pixel goes as the data is decoded.
struct Cursor<'a> {
    framebuffer: &'a Framebuffer,
    left: usize,
    top: usize,
    width: usize,
    /// How many pixels of the image have been drawn so far.
    drawn: usize,
    total: usize,
}

impl Cursor<'_> {
    /// Draws `count` copies of a color, splitting them across rows as needed.
    fn run(&mut self, mut count: usize, color: Option<Color>) -> Result<(), BadImage> {
        if self.drawn + count > self.total {
            return Err(BadImage);
        }
        while count > 0 {
            let column = self.drawn % self.width;
            let row = self.drawn / self.width;
            let span = count.min(self.width - column);
            if let Some(color) = color {
                self.framebuffer.fill_rect(self.left + column, self.top + row, span, 1, color);
            }
            self.drawn += span;
            count -= span;
        }
        Ok(())
    }

    fn raw(&mut self, symbols: &[u8], format: &Symbols) -> Result<(), BadImage> {
        if self.drawn + symbols.len() / format.size() > self.total {
            return Err(BadImage);
        }
        for symbol in symbols.chunks_exact(format.size()) {
            if let Some(color) = format.color(symbol)? {
                let column = self.drawn % self.width;
                let row = self.drawn / self.width;
                self.framebuffer.put_row(self.left + column, self.top + row, iter::once(color));
            }
            self.drawn += 1;
        }
        Ok(())
    }
}

/// Draws an image with its top-left corner at `left`, `top`. Anything off the
/// screen is clipped.
pub fn draw(framebuffer: &Framebuffer, left: usize, top: usize, data: &[u8]) -> Result<(), BadImage> {
    let header = data.get(..HEADER_BYTES).ok_or(BadImage)?;
    let width = usize::from(u16::from_le_bytes([header[0], header[1]]));
    let height = usize::from(u16::from_le_bytes([header[2], header[3]]));
    let index_size = usize::from(header[INDEX_SIZE_AT]);
    let palette_count = usize::from(u16::from_le_bytes([header[PALETTE_COUNT_AT], header[PALETTE_COUNT_AT + 1]]));
    if width == 0 || index_size > 2 {
        return Err(BadImage);
    }

    let packets_at = HEADER_BYTES + palette_count * PALETTE_ENTRY_BYTES;
    let palette = data.get(HEADER_BYTES..packets_at).ok_or(BadImage)?;
    let format = Symbols { index_size, palette };
    let size = format.size();

    let mut cursor = Cursor { framebuffer, left, top, width, drawn: 0, total: width * height };
    let mut at = packets_at;
    while at < data.len() {
        let first = data[at];
        if first == ESCAPE {
            let length = *data.get(at + 1).ok_or(BadImage)?;
            let count = if length == 0 { MAX_RAW } else { usize::from(length) };
            let end = at + 2 + count * size;
            cursor.raw(data.get(at + 2..end).ok_or(BadImage)?, &format)?;
            at = end;
        } else {
            let end = at + 1 + size;
            let symbol = data.get(at + 1..end).ok_or(BadImage)?;
            cursor.run(usize::from(first), format.color(symbol)?)?;
            at = end;
        }
    }
    if cursor.drawn == cursor.total {
        Ok(())
    } else {
        Err(BadImage)
    }
}
