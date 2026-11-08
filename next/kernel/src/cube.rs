//! A spinning cube drawn by the CPU, with fixed-point math. It draws into a buffer in
//! cached memory and copies that to the screen in one go, so it can't flicker, and it's
//! a baseline for what the GPU can do.

use core::slice;

use crate::board;
use crate::pictures::{LOGO_CENTER_Y, LOGO_HEIGHT, LOGO_LEFT, LOGO_WIDTH};
use crate::video::{Color, Framebuffer, BLACK};

/// The cube's square of the screen: where the logo turns, over Aki's hand, as wide as the
/// logo facing front, which it takes the place of. That's inside the logo's own box, so
/// it touches neither Aki nor the square circling it.
pub const SIZE: usize = LOGO_WIDTH;
pub const LEFT: usize = LOGO_LEFT;
pub const TOP: usize = LOGO_CENTER_Y - SIZE / 2;
pub const HALF: i32 = (SIZE / 2) as i32;
const _: () = assert!(SIZE <= LOGO_HEIGHT);

/// Numbers with this many fractional bits.
pub const FRACTION_BITS: u32 = 14;
const ONE: i32 = 1 << FRACTION_BITS;
/// How far the camera is from the cube's center, in cube half-widths.
pub const CAMERA_DISTANCE: i32 = 4 * ONE;
/// Fits the corners, at most 1.41 half-widths out and 3 from the camera, inside the square:
/// 1.9 half-widths, so a corner reaches 0.89 of the way to the edge.
pub const FOCAL_PIXELS: i32 = HALF * 19 / 10;

/// A full turn, in the units angles are kept in.
pub const TURN: i32 = 1024;
const SPIN_X_PER_FRAME: i32 = 3;
const SPIN_Y_PER_FRAME: i32 = 5;
const SPIN_Z_PER_FRAME: i32 = 2;

const MIN_BRIGHTNESS: i32 = 70;
const MAX_BRIGHTNESS: i32 = 255;
const BYTE_BITS: u32 = 8;
const OPAQUE: Color = 0xFF00_0000;

pub type Vector = [i32; 3];

pub const CORNERS: [Vector; 8] = [
    [-ONE, -ONE, -ONE],
    [ONE, -ONE, -ONE],
    [ONE, ONE, -ONE],
    [-ONE, ONE, -ONE],
    [-ONE, -ONE, ONE],
    [ONE, -ONE, ONE],
    [ONE, ONE, ONE],
    [-ONE, ONE, ONE],
];

/// A face's corners, clockwise as seen from outside, its outward direction and its color.
pub struct Face {
    pub corners: [usize; 4],
    pub normal: Vector,
    pub rgb: [i32; 3],
}

pub const FACES: [Face; 6] = [
    Face { corners: [0, 1, 2, 3], normal: [0, 0, -ONE], rgb: [255, 96, 0] },
    Face { corners: [5, 4, 7, 6], normal: [0, 0, ONE], rgb: [255, 64, 0] },
    Face { corners: [4, 0, 3, 7], normal: [-ONE, 0, 0], rgb: [255, 255, 255] },
    Face { corners: [1, 5, 6, 2], normal: [ONE, 0, 0], rgb: [64, 180, 160] },
    Face { corners: [4, 5, 1, 0], normal: [0, -ONE, 0], rgb: [240, 170, 170] },
    Face { corners: [3, 2, 6, 7], normal: [0, ONE, 0], rgb: [150, 130, 100] },
];

/// How far the cube has turned at a frame.
pub const fn angles_at(frame: i32) -> Vector {
    [frame * SPIN_X_PER_FRAME, frame * SPIN_Y_PER_FRAME, frame * SPIN_Z_PER_FRAME]
}

/// A sine approximation good enough to look right (Bhaskara I's), for `angle` in `TURN`s.
fn sine(angle: i32) -> i32 {
    let half = TURN / 2;
    let wrapped = angle.rem_euclid(TURN);
    let (position, sign) = if wrapped < half { (wrapped, 1) } else { (wrapped - half, -1) };
    let product = i64::from(position) * i64::from(half - position);
    let quarter_square = i64::from(half) * i64::from(half);
    let value = 16 * product * i64::from(ONE) / (5 * quarter_square - 4 * product);
    sign * value as i32
}

fn cosine(angle: i32) -> i32 {
    sine(angle + TURN / 4)
}

fn multiply(a: i32, b: i32) -> i32 {
    ((i64::from(a) * i64::from(b)) >> FRACTION_BITS) as i32
}

/// Rotates around X, then Y, then Z.
pub fn rotate(point: Vector, angles: Vector) -> Vector {
    let [x, y, z] = point;
    let (sin, cos) = (sine(angles[0]), cosine(angles[0]));
    let (y, z) = (multiply(y, cos) - multiply(z, sin), multiply(y, sin) + multiply(z, cos));
    let (sin, cos) = (sine(angles[1]), cosine(angles[1]));
    let (x, z) = (multiply(x, cos) + multiply(z, sin), multiply(z, cos) - multiply(x, sin));
    let (sin, cos) = (sine(angles[2]), cosine(angles[2]));
    let (x, y) = (multiply(x, cos) - multiply(y, sin), multiply(x, sin) + multiply(y, cos));
    [x, y, z]
}

/// A point's position in the square, in whole pixels.
fn project(point: Vector) -> (i32, i32) {
    let distance = i64::from(point[2] + CAMERA_DISTANCE);
    let scale = |value: i32| (i64::from(value) * i64::from(FOCAL_PIXELS) / distance) as i32;
    (HALF + scale(point[0]), HALF + scale(point[1]))
}

/// Twice the triangle's signed area, which tells which side of an edge a point is on.
fn edge(a: (i32, i32), b: (i32, i32), point: (i32, i32)) -> i32 {
    (b.0 - a.0) * (point.1 - a.1) - (b.1 - a.1) * (point.0 - a.0)
}

/// The square's pixels. Too big for the stack, and only the cube uses it. It starts as
/// zeros so that it stays out of the kernel's file.
static mut BUFFER: [Color; SIZE * SIZE] = [0; SIZE * SIZE];

fn fill_triangle(buffer: &mut [Color], a: (i32, i32), b: (i32, i32), c: (i32, i32), color: Color) {
    let left = a.0.min(b.0).min(c.0).max(0);
    let right = a.0.max(b.0).max(c.0).min(SIZE as i32 - 1);
    let top = a.1.min(b.1).min(c.1).max(0);
    let bottom = a.1.max(b.1).max(c.1).min(SIZE as i32 - 1);
    for y in top..=bottom {
        let row = y as usize * SIZE;
        for x in left..=right {
            let point = (x, y);
            if edge(a, b, point) >= 0 && edge(b, c, point) >= 0 && edge(c, a, point) >= 0 {
                buffer[row + x as usize] = color;
            }
        }
    }
}

pub fn shade(rgb: [i32; 3], facing: i32) -> Color {
    let brightness = MIN_BRIGHTNESS + (MAX_BRIGHTNESS - MIN_BRIGHTNESS) * facing.max(0) / ONE;
    let channel = |value: i32| (value * brightness / MAX_BRIGHTNESS) as Color;
    OPAQUE | channel(rgb[0]) << (2 * BYTE_BITS) | channel(rgb[1]) << BYTE_BITS | channel(rgb[2])
}

pub struct Cube {
    frame: i32,
    frames_timed: u32,
    microseconds_drawing: u64,
}

impl Cube {
    pub const fn new() -> Self {
        Self { frame: 0, frames_timed: 0, microseconds_drawing: 0 }
    }

    /// Draws the next frame.
    pub fn draw(&mut self, framebuffer: &Framebuffer) {
        let start = board::microseconds();
        // SAFETY: only this runs on this core, and nothing else uses the buffer.
        let buffer = unsafe { slice::from_raw_parts_mut((&raw mut BUFFER).cast::<Color>(), SIZE * SIZE) };
        buffer.fill(BLACK);

        let angles = angles_at(self.frame);
        let projected = CORNERS.map(|corner| project(rotate(corner, angles)));
        for face in &FACES {
            let [a, b, c, d] = face.corners.map(|corner| projected[corner]);
            // A cube is convex, so hiding the faces turned away is all the depth it needs.
            if edge(a, b, c) <= 0 {
                continue;
            }
            let facing = -rotate(face.normal, angles)[2];
            let color = shade(face.rgb, facing);
            fill_triangle(buffer, a, b, c, color);
            fill_triangle(buffer, a, c, d, color);
        }

        for (row, pixels) in buffer.chunks_exact(SIZE).enumerate() {
            framebuffer.put_row(LEFT, TOP + row, pixels.iter().copied());
        }

        self.frame = (self.frame + 1) % TURN;
        self.frames_timed += 1;
        self.microseconds_drawing += u64::from(board::microseconds().wrapping_sub(start));
    }

    /// Erases the cube, and reports how long its frames took on average.
    pub fn stop(&mut self, framebuffer: &Framebuffer) {
        framebuffer.fill_rect(LEFT, TOP, SIZE, SIZE, BLACK);
        if let Some(average) = self.microseconds_drawing.checked_div(u64::from(self.frames_timed)) {
            println!("Cube: {} frames, {average} us each.", self.frames_timed);
        }
        self.frames_timed = 0;
        self.microseconds_drawing = 0;
    }
}
