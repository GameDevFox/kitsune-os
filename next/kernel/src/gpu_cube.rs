//! The same spinning cube as `cube.rs`, drawn by the GPU. All 36 vertices go to it, with
//! nothing hidden first, so that the depth test decides what's in front.

use crate::board;
use crate::board::gpu::{fixed_bits, Scene, Vertex};
use crate::cube::{angles_at, rotate, shade, CAMERA_DISTANCE, CORNERS, FACES, FOCAL_PIXELS, FRACTION_BITS, HALF, LEFT, TOP, TURN};
use crate::video::{Color, Framebuffer, BLACK};

const TRIANGLES_PER_FACE: usize = 2;
const VERTICES_PER_TRIANGLE: usize = 3;
const VERTEX_COUNT: usize = FACES.len() * TRIANGLES_PER_FACE * VERTICES_PER_TRIANGLE;
/// A face's two triangles, as positions in its list of corners.
const FACE_TRIANGLES: [usize; TRIANGLES_PER_FACE * VERTICES_PER_TRIANGLE] = [0, 1, 2, 0, 2, 3];
/// Depth is z in half-widths, halved, to stay within -1 to 1 for the corners' 1.73.
const DEPTH_SHIFT: u32 = 1;
const BYTE_MASK: Color = 0xFF;
const RED_SHIFT: u32 = 16;
const GREEN_SHIFT: u32 = 8;
const OPAQUE: u8 = 0xFF;

// The GPU fills its whole area, so it must be exactly the cube's.
const _: () = assert!(board::gpu::WIDTH == crate::cube::SIZE && board::gpu::HEIGHT == crate::cube::SIZE);

pub struct GpuCube {
    scene: Scene,
    frame: i32,
    frames_timed: u32,
    microseconds_drawing: u64,
}

impl GpuCube {
    /// Returns `None` if the GPU doesn't answer.
    pub fn new(framebuffer: &Framebuffer) -> Option<Self> {
        let scene = Scene::new(framebuffer, LEFT, TOP, VERTEX_COUNT as u32)?;
        Some(Self { scene, frame: 0, frames_timed: 0, microseconds_drawing: 0 })
    }

    /// Draws the next frame. Returns false if the GPU timed out.
    pub fn draw(&mut self) -> bool {
        let start = board::microseconds();
        let angles = angles_at(self.frame);
        let rotated = CORNERS.map(|corner| rotate(corner, angles));
        for (face_number, face) in FACES.iter().enumerate() {
            let color = shade(face.rgb, -rotate(face.normal, angles)[2]);
            let color = [(color >> RED_SHIFT & BYTE_MASK) as u8, (color >> GREEN_SHIFT & BYTE_MASK) as u8, (color & BYTE_MASK) as u8, OPAQUE];
            for (index, position) in FACE_TRIANGLES.iter().enumerate() {
                let [x, y, z] = rotated[face.corners[*position]];
                let distance = i64::from(z + CAMERA_DISTANCE);
                let scale = |value: i32| (((i64::from(value) * i64::from(FOCAL_PIXELS)) << FRACTION_BITS) / distance / i64::from(HALF)) as i32;
                let position = [fixed_bits(scale(x), FRACTION_BITS), fixed_bits(-scale(y), FRACTION_BITS), fixed_bits(z >> DEPTH_SHIFT, FRACTION_BITS)];
                self.scene.set_vertex(face_number * FACE_TRIANGLES.len() + index, &Vertex { position, color });
            }
        }
        let good = self.scene.render();
        self.frame = (self.frame + 1) % TURN;
        self.frames_timed += 1;
        self.microseconds_drawing += u64::from(board::microseconds().wrapping_sub(start));
        good
    }

    /// Erases the cube, and reports how long its frames took on average.
    pub fn stop(&mut self, framebuffer: &Framebuffer) {
        framebuffer.fill_rect(LEFT, TOP, crate::cube::SIZE, crate::cube::SIZE, BLACK);
        if let Some(average) = self.microseconds_drawing.checked_div(u64::from(self.frames_timed)) {
            println!("GPU cube: {} frames, {average} us each.", self.frames_timed);
        }
    }
}
