//! The logo turning about its vertical axis, in perspective, so the edge coming towards
//! the viewer grows and the far one shrinks. Without that, the picture goes still for a
//! moment whenever it faces the viewer squarely, which made one turn look like two flips.
//! Each frame, every column of the logo's box works out which column of the logo lands
//! there and how near it is, which sets its height, and every pixel takes its nearest
//! source pixel, with no blending. It makes one full turn at a steady speed, starting
//! and stopping at once, then rests facing front, and repeats. `tools/check_images.py`
//! repeats this arithmetic exactly, to compare QEMU's screen, so keep the two in step.

use core::cell::UnsafeCell;

use crate::trig::{cos, sin, FRACTION_BITS, ONE, STEPS};
use crate::pictures::{LOGO_HEIGHT, LOGO_LEFT, LOGO_TOP, LOGO_WIDTH};
use crate::video::{Color, Framebuffer, BLACK};

/// At 60 frames a second, the turn takes 3 s, and the rest after it 2 s.
const TURN_FRAMES: u64 = 180;
const REST_FRAMES: u64 = 120;

/// How far away the viewer is, in pixels, from the axis it turns on. Nearer is more
/// perspective: at this distance, its near edge grows by about 9% at most.
const VIEW_DISTANCE: i64 = 1000;
/// Rows above and below the logo's box that its near edge can reach. The square's circle
/// clears them by 8 px, as only the columns near the middle ever get that tall.
pub const EXTRA_ROWS: usize = 12;
const _: () = assert!(
    (LOGO_HEIGHT / 2) as i64 * VIEW_DISTANCE <= (LOGO_HEIGHT / 2 + EXTRA_ROWS) as i64 * (VIEW_DISTANCE - (LOGO_WIDTH / 2) as i64)
);
const DRAWN_ROWS: usize = LOGO_HEIGHT + 2 * EXTRA_ROWS;


const PIXELS: usize = LOGO_WIDTH * LOGO_HEIGHT;

/// The logo as it was first drawn, black where it's clear, to read from every frame.
struct Original(UnsafeCell<[Color; PIXELS]>);

// SAFETY: only the main loop uses this, on one core, and interrupt handlers never do.
unsafe impl Sync for Original {}

// All zeros, which is clear rather than black, so it goes in .bss, which boot zeroes,
// rather than adding 172 KB to the kernel image and to every load. `capture` fills it.
static ORIGINAL: Original = Original(UnsafeCell::new([0; PIXELS]));

/// Where one column of the box reads from: a column of the logo, and how much nearer
/// or further than the axis that column is, as a scale with `FRACTION_BITS`, so heights
/// shrink by it.
#[derive(Clone, Copy)]
struct Source {
    column: usize,
    depth: i64,
}

impl Source {
    /// The logo's row that lands on a row of the drawn rows, if any. Positions are
    /// measured from the center in half pixels, so they stay whole.
    fn row(&self, drawn_row: usize) -> Option<usize> {
        let height = LOGO_HEIGHT as i64;
        let from_center = 2 * drawn_row as i64 + 1 - DRAWN_ROWS as i64;
        let unscaled = (from_center * self.depth) >> FRACTION_BITS;
        (unscaled.abs() < height).then(|| ((unscaled + height) / 2) as usize)
    }
}

type Sources = [Option<Source>; LOGO_WIDTH];

pub struct Spinner {
    /// Frames into the turn and rest. It rests from `TURN_FRAMES` on, facing front.
    frame: u64,
    original: &'static mut [Color; PIXELS],
    /// Where each column read from last time, so what it drew can be cleared.
    previous: Sources,
    /// Off the screen, while something else has its place.
    hidden: bool,
}

impl Spinner {
    /// Copies the logo off the screen, where it has just been drawn. Only one may exist.
    pub fn capture(framebuffer: &Framebuffer) -> Self {
        // SAFETY: see `Original`, and this is the only place that hands it out.
        let original = unsafe { &mut *ORIGINAL.0.get() };
        for (row, pixels) in original.chunks_exact_mut(LOGO_WIDTH).enumerate() {
            framebuffer.get_row(LOGO_LEFT, LOGO_TOP + row, pixels);
        }
        Self { frame: 0, original, previous: sources(0), hidden: false }
    }

    /// Moves on a frame and draws the logo at its new angle. While it rests, nothing
    /// changes, so it draws only on the first frame, the turn's last, facing front.
    pub fn step_and_draw(&mut self, framebuffer: &Framebuffer) {
        if self.hidden {
            return;
        }
        self.frame = (self.frame + 1) % (TURN_FRAMES + REST_FRAMES);
        if self.frame > TURN_FRAMES {
            return;
        }
        self.draw(framebuffer, sources((STEPS as u64 * self.frame / TURN_FRAMES) as i64));
    }

    /// Erases the logo, wherever it reached, and stops it until `show`.
    pub fn hide(&mut self, framebuffer: &Framebuffer) {
        if !self.hidden {
            self.draw(framebuffer, [None; LOGO_WIDTH]);
            self.hidden = true;
        }
    }

    /// Brings the logo back facing front, to rest and then turn as usual.
    pub fn show(&mut self, framebuffer: &Framebuffer) {
        if self.hidden {
            self.hidden = false;
            self.frame = TURN_FRAMES;
            self.draw(framebuffer, sources(0));
        }
    }

    /// Draws the logo as `sources` say, a row at a time, so there's no blank moment to
    /// flicker. The logo's own rows are drawn right across its box. In the rows above and
    /// below, only columns that have the logo now or had it last time are touched, which
    /// keeps clear of the square.
    fn draw(&mut self, framebuffer: &Framebuffer, sources: Sources) {
        let top = LOGO_TOP - EXTRA_ROWS;
        let inside_box = EXTRA_ROWS..EXTRA_ROWS + LOGO_HEIGHT;
        for drawn_row in 0..DRAWN_ROWS {
            let color = |source: &Option<Source>| match source.and_then(|source| Some((source.column, source.row(drawn_row)?))) {
                Some((column, row)) => self.original[row * LOGO_WIDTH + column],
                None => BLACK,
            };
            if inside_box.contains(&drawn_row) {
                framebuffer.put_row(LOGO_LEFT, top + drawn_row, sources.iter().map(color));
                continue;
            }
            let touched = |column: usize| {
                [sources[column], self.previous[column]].iter().any(|source| source.is_some_and(|source| source.row(drawn_row).is_some()))
            };
            let mut column = 0;
            while column < LOGO_WIDTH {
                if !touched(column) {
                    column += 1;
                    continue;
                }
                let start = column;
                while column < LOGO_WIDTH && touched(column) {
                    column += 1;
                }
                framebuffer.put_row(LOGO_LEFT + start, top + drawn_row, sources[start..column].iter().map(color));
            }
        }
        self.previous = sources;
    }
}

/// Where each column of the box reads from, with the logo turned `angle` steps. A point
/// `x` from the axis lands at `x cos / (1 + x sin / distance)`, so this solves that for
/// `x`, in half pixels from the center, so they stay whole.
fn sources(angle: i64) -> Sources {
    let (cosine, sine) = (cos(angle), sin(angle));
    let width = LOGO_WIDTH as i64;
    let mut sources = [None; LOGO_WIDTH];
    for (column, source) in sources.iter_mut().enumerate() {
        let from_center = 2 * column as i64 + 1 - width;
        let below = 2 * VIEW_DISTANCE * cosine - from_center * sine;
        if below == 0 {
            continue;
        }
        let unsqueezed = from_center * 2 * VIEW_DISTANCE * ONE / below;
        if unsqueezed.abs() < width {
            let depth = ONE + unsqueezed * sine / (2 * VIEW_DISTANCE);
            *source = Some(Source { column: ((unsqueezed + width) / 2) as usize, depth });
        }
    }
    sources
}
