//! The lists of commands the GPU runs, built once and then reused for every frame.

use core::ptr::write_volatile;

use super::shaders::{COORDINATE_SHADER, FRAGMENT_SHADER, VERTEX_SHADER};
use super::{float_bits, FLOAT_HALF, FLOAT_ONE, FLOAT_ZERO, MEMORY_BASE, MEMORY_BYTES};
use crate::board::UNCACHED_VIEW;

/// A piece of GPU memory being written in order, one list or table at a time.
pub struct Writer {
    pub next: usize,
}

impl Writer {
    /// Moves to the next address with this alignment, and returns it.
    pub fn align(&mut self, alignment: usize) -> u32 {
        self.next = self.next.next_multiple_of(alignment);
        assert!(self.next < MEMORY_BASE + MEMORY_BYTES);
        self.next as u32
    }

    pub fn u8(&mut self, value: u8) -> &mut Self {
        // SAFETY: inside the memory set aside above, through the uncached view.
        unsafe { write_volatile((self.next + UNCACHED_VIEW) as *mut u8, value) };
        self.next += 1;
        self
    }

    pub fn bytes(&mut self, values: &[u8]) -> &mut Self {
        for value in values {
            self.u8(*value);
        }
        self
    }

    pub fn u16(&mut self, value: u16) -> &mut Self {
        self.bytes(&value.to_le_bytes())
    }

    pub fn u32(&mut self, value: u32) -> &mut Self {
        self.bytes(&value.to_le_bytes())
    }

    pub fn u64(&mut self, value: u64) -> &mut Self {
        self.bytes(&value.to_le_bytes())
    }

    pub fn reserve(&mut self, bytes: usize, alignment: usize) -> u32 {
        let start = self.align(alignment);
        self.next += bytes;
        assert!(self.next <= MEMORY_BASE + MEMORY_BYTES);
        start
    }
}

// Command list opcodes, from Mesa's `v3d_packet.xml`.
const OP_FLUSH: u8 = 4;
const OP_START_TILE_BINNING: u8 = 6;
const OP_END_OF_RENDERING: u8 = 13;
const OP_RETURN_FROM_SUB_LIST: u8 = 18;
const OP_FLUSH_VCD_CACHE: u8 = 19;
const OP_START_ADDRESS_OF_GENERIC_TILE_LIST: u8 = 20;
const OP_BRANCH_TO_IMPLICIT_TILE_LIST: u8 = 21;
const OP_SUPERTILE_COORDINATES: u8 = 23;
const OP_CLEAR_TILE_BUFFERS: u8 = 25;
const OP_END_OF_LOADS: u8 = 26;
const OP_END_OF_TILE_MARKER: u8 = 27;
const OP_STORE_TILE_BUFFER_GENERAL: u8 = 29;
const OP_VERTEX_ARRAY_PRIMS: u8 = 36;
const OP_PRIM_LIST_FORMAT: u8 = 56;
const OP_GL_SHADER_STATE: u8 = 64;
const OP_VCM_CACHE_SIZE: u8 = 71;
const OP_TRANSFORM_FEEDBACK_SPECS: u8 = 74;
const OP_BLEND_CONSTANT_COLOR: u8 = 86;
const OP_COLOR_WRITE_MASKS: u8 = 87;
const OP_ZERO_ALL_CENTROID_FLAGS: u8 = 88;
const OP_SAMPLE_STATE: u8 = 91;
const OP_OCCLUSION_QUERY_COUNTER: u8 = 92;
const OP_CFG_BITS: u8 = 96;
const OP_ZERO_ALL_FLAT_SHADE_FLAGS: u8 = 97;
const OP_ZERO_ALL_NON_PERSPECTIVE_FLAGS: u8 = 99;
const OP_POINT_SIZE: u8 = 104;
const OP_LINE_WIDTH: u8 = 105;
const OP_CLIP_WINDOW: u8 = 107;
const OP_VIEWPORT_OFFSET: u8 = 108;
const OP_CLIPPER_Z_MIN_MAX_CLIPPING_PLANES: u8 = 109;
const OP_CLIPPER_XY_SCALING: u8 = 110;
const OP_CLIPPER_Z_SCALE_AND_OFFSET: u8 = 111;
const OP_TILE_BINNING_MODE_CFG: u8 = 120;
const OP_TILE_RENDERING_MODE_CFG: u8 = 121;
const OP_MULTICORE_RENDERING_SUPERTILE_CFG: u8 = 122;
const OP_MULTICORE_RENDERING_TILE_LIST_SET_BASE: u8 = 123;
const OP_TILE_COORDINATES: u8 = 124;
const OP_TILE_COORDINATES_IMPLICIT: u8 = 125;
const OP_TILE_LIST_INITIAL_BLOCK_SIZE: u8 = 126;

const SUB_COMMON: u8 = 0;
const SUB_COLOR: u64 = 1;
const SUB_ZS_CLEAR: u8 = 2;
const SUB_CLEAR_COLOR: u64 = 3;

/// The area drawn, which `gpu_cube.rs` checks is the cube's. The GPU fills all of it,
/// so a bigger one would paint over whatever is around the cube.
pub const WIDTH: usize = 168;
pub const HEIGHT: usize = 168;
const TILE_PIXELS: usize = 64;
const TILES_X: usize = WIDTH.div_ceil(TILE_PIXELS);
const TILES_Y: usize = HEIGHT.div_ceil(TILE_PIXELS);
pub const TILE_ALLOCATION_BYTES: usize = (TILES_X * TILES_Y * 64).next_multiple_of(4096) + 2 * 64 * 64;
const TILE_STATE_BYTES: usize = TILES_X * TILES_Y * 256;
const PAGE_BYTES: usize = 4096;
/// The clipper and shaders work in 1/256ths of a pixel.
const SUBPIXELS: i32 = 256;
/// Opaque black, as the bytes the tile buffer holds.
const CLEAR_COLOR: u64 = 0xFF00_0000;
const DEFAULT_ATTRIBUTE_BYTES: usize = 64 * 4;
const PRIMITIVE_TRIANGLES: u8 = 4;
const LIST_TRIANGLES: u8 = 2;
const BUFFER_RENDER_TARGET_0: u8 = 0;
const BUFFER_NONE: u8 = 8;
const OUTPUT_FORMAT_RGBA8: u16 = 27;
const STORE_FORMAT_SHIFT: u32 = 4;
/// Stores red and blue swapped, which suits the screen's blue-first pixels.
const STORE_RED_BLUE_SWAP: u16 = 1 << 12;
const STORE_STRIDE_SHIFT: u32 = 4;
const SHADER_FLAGS_VERTEX: u32 = 0b111;
const SHADER_FLAGS_FRAGMENT: u32 = 0b101;
const SHADER_ALIGNMENT: usize = 8;
const RECORD_ALIGNMENT: usize = 32;
const WORD_ALIGNMENT: usize = 4;
const LIST_ALIGNMENT: usize = 64;
const POSITION_BYTES: usize = 3 * 4;
const COLOR_BYTES: usize = 4;
const MAX_INDEX: u32 = 0xFF_FFFF;
const VIEWPORT_CENTER_X: u32 = (WIDTH / 2) as u32 * SUBPIXELS as u32;
const VIEWPORT_CENTER_Y: u32 = (HEIGHT / 2) as u32 * SUBPIXELS as u32;
const SCALE_X: u32 = float_bits(WIDTH as i32 / 2 * SUBPIXELS);
const SCALE_Y: u32 = float_bits(-(HEIGHT as i32 / 2 * SUBPIXELS));

/// Where everything is, once built.
pub struct Lists {
    pub bin_start: u32,
    pub bin_end: u32,
    pub render_start: u32,
    pub render_end: u32,
    pub tile_allocation: u32,
    pub tile_state: u32,
    pub positions: u32,
    pub colors: u32,
}

fn write_shader(writer: &mut Writer, code: &[u64]) -> u32 {
    let address = writer.align(SHADER_ALIGNMENT);
    for instruction in code {
        writer.u64(*instruction);
    }
    address
}

fn write_store(memory: &mut Writer, buffer: u8, flags: u16, stride: u32, address: u32) {
    memory.bytes(&[OP_STORE_TILE_BUFFER_GENERAL, buffer]).u16(flags);
    let [low, middle, high, _] = (stride << STORE_STRIDE_SHIFT).to_le_bytes();
    memory.bytes(&[low, middle, high]).u16(0).u32(address);
}

/// Writes the lists to draw `vertex_count` vertices as triangles, with a depth test, into
/// the image at physical address `target`, whose rows are `stride` bytes apart. The depth
/// buffer only exists inside the GPU's tile buffer, so it needs no memory of its own.
pub fn build(target: u32, stride: u32, vertex_count: u32) -> Lists {
    let mut writer = Writer { next: MEMORY_BASE };
    let memory = &mut writer;

    let positions = memory.reserve(vertex_count as usize * POSITION_BYTES, LIST_ALIGNMENT);
    let colors = memory.reserve(vertex_count as usize * COLOR_BYTES, LIST_ALIGNMENT);

    let default_attributes = memory.align(LIST_ALIGNMENT);
    for _ in 0..DEFAULT_ATTRIBUTE_BYTES / 16 {
        memory.u32(FLOAT_ZERO).u32(FLOAT_ZERO).u32(FLOAT_ZERO).u32(FLOAT_ONE);
    }

    let fragment_shader = write_shader(memory, &FRAGMENT_SHADER);
    let vertex_shader = write_shader(memory, &VERTEX_SHADER);
    let coordinate_shader = write_shader(memory, &COORDINATE_SHADER);

    // Uniforms. The fragment shader reads none.
    let fragment_uniforms = memory.align(WORD_ALIGNMENT);
    let vertex_uniforms = memory.align(WORD_ALIGNMENT);
    memory.u32(FLOAT_ONE).u32(SCALE_X).u32(SCALE_Y).u32(FLOAT_HALF).u32(FLOAT_HALF);
    let coordinate_uniforms = memory.align(WORD_ALIGNMENT);
    memory.u32(FLOAT_ONE).u32(SCALE_X).u32(SCALE_Y);

    // The shader state record, and its two attribute records straight after it.
    let shader_state = memory.align(RECORD_ALIGNMENT);
    memory.bytes(&[0b10, 0b1_0000, 0b100, 4]); // Clipping, real pixel center W, no implicit varyings, 4 varyings.
    memory.bytes(&[1, 1, 1, 1]); // VPM segment sizes.
    memory.u32(default_attributes);
    memory.u32(fragment_shader | SHADER_FLAGS_FRAGMENT).u32(fragment_uniforms);
    memory.u32(vertex_shader | SHADER_FLAGS_VERTEX).u32(vertex_uniforms);
    memory.u32(coordinate_shader | SHADER_FLAGS_VERTEX).u32(coordinate_uniforms);
    // Position: 3 floats, read by both shaders. Color: 4 normalized bytes, for the vertex one.
    memory.u32(positions).bytes(&[0b0000_1011, 0x33]).u16(0).u32(POSITION_BYTES as u32).u32(MAX_INDEX);
    memory.u32(colors).bytes(&[0b0101_0000, 0x40]).u16(0).u32(COLOR_BYTES as u32).u32(MAX_INDEX);
    const ATTRIBUTE_COUNT: u32 = 2;

    let tile_allocation = memory.reserve(TILE_ALLOCATION_BYTES, PAGE_BYTES);
    let tile_state = memory.reserve(TILE_STATE_BYTES, PAGE_BYTES);

    // The binning list.
    let bin_start = memory.align(LIST_ALIGNMENT);
    memory.bytes(&[OP_TILE_BINNING_MODE_CFG, 0, 0, 0, 0]).u16(WIDTH as u16 - 1).u16(HEIGHT as u16 - 1);
    memory.u8(OP_FLUSH_VCD_CACHE);
    memory.u8(OP_OCCLUSION_QUERY_COUNTER).u32(0);
    memory.u8(OP_START_TILE_BINNING);
    memory.u8(OP_CLIP_WINDOW).u16(0).u16(0).u16(WIDTH as u16).u16(HEIGHT as u16);
    // Both faces, then a "less than" depth test with depth updates, early where it can be.
    memory.bytes(&[OP_CFG_BITS, 0b0000_0111, 0b1001_0000, 0b11]);
    memory.u8(OP_POINT_SIZE).u32(FLOAT_ONE);
    memory.u8(OP_LINE_WIDTH).u32(FLOAT_ONE);
    memory.u8(OP_CLIPPER_XY_SCALING).u32(SCALE_X).u32(SCALE_Y);
    memory.u8(OP_CLIPPER_Z_SCALE_AND_OFFSET).u32(FLOAT_HALF).u32(FLOAT_HALF);
    memory.u8(OP_CLIPPER_Z_MIN_MAX_CLIPPING_PLANES).u32(FLOAT_ZERO).u32(FLOAT_ONE);
    memory.u8(OP_VIEWPORT_OFFSET).u32(VIEWPORT_CENTER_X).u32(VIEWPORT_CENTER_Y);
    memory.u8(OP_COLOR_WRITE_MASKS).u32(0);
    memory.u8(OP_BLEND_CONSTANT_COLOR).u16(0).u16(0).u16(0).u16(0);
    memory.u8(OP_ZERO_ALL_FLAT_SHADE_FLAGS);
    memory.u8(OP_ZERO_ALL_NON_PERSPECTIVE_FLAGS);
    memory.u8(OP_ZERO_ALL_CENTROID_FLAGS);
    memory.bytes(&[OP_TRANSFORM_FEEDBACK_SPECS, 0]);
    memory.u8(OP_OCCLUSION_QUERY_COUNTER).u32(0);
    memory.bytes(&[OP_SAMPLE_STATE, 0xF, 0]).u16(0x3F80);
    memory.bytes(&[OP_VCM_CACHE_SIZE, 0x44]);
    memory.u8(OP_GL_SHADER_STATE).u32(shader_state | ATTRIBUTE_COUNT);
    memory.bytes(&[OP_VERTEX_ARRAY_PRIMS, PRIMITIVE_TRIANGLES]).u32(vertex_count).u32(0);
    memory.u8(OP_FLUSH);
    let bin_end = memory.next as u32;

    // What each tile runs.
    let tile_list_start = memory.align(LIST_ALIGNMENT);
    memory.u8(OP_TILE_COORDINATES_IMPLICIT);
    memory.u8(OP_END_OF_LOADS);
    memory.bytes(&[OP_PRIM_LIST_FORMAT, LIST_TRIANGLES]);
    memory.bytes(&[OP_BRANCH_TO_IMPLICIT_TILE_LIST, 0]);
    write_store(memory, BUFFER_RENDER_TARGET_0, OUTPUT_FORMAT_RGBA8 << STORE_FORMAT_SHIFT | STORE_RED_BLUE_SWAP, stride, target);
    memory.bytes(&[OP_CLEAR_TILE_BUFFERS, 0b11]);
    memory.u8(OP_END_OF_TILE_MARKER);
    memory.u8(OP_RETURN_FROM_SUB_LIST);
    let tile_list_end = memory.next as u32;

    // The rendering list.
    let render_start = memory.align(LIST_ALIGNMENT);
    memory.bytes(&[OP_TILE_RENDERING_MODE_CFG, SUB_COMMON]).u16(WIDTH as u16).u16(HEIGHT as u16);
    memory.u16(2 << 7).u8(0); // 16-bit depth inside the tile buffer.
    memory.u8(OP_TILE_RENDERING_MODE_CFG).u64(SUB_CLEAR_COLOR | CLEAR_COLOR << 8);
    memory.u8(OP_TILE_RENDERING_MODE_CFG).u64(SUB_COLOR | 2 << 6); // 32 bits per pixel, 8 per channel.
    memory.bytes(&[OP_TILE_RENDERING_MODE_CFG, SUB_ZS_CLEAR, 0]).u32(FLOAT_ONE).u16(0);
    memory.bytes(&[OP_TILE_LIST_INITIAL_BLOCK_SIZE, 0b100]); // 64-byte blocks, chained automatically.
    memory.u8(OP_MULTICORE_RENDERING_TILE_LIST_SET_BASE).u32(tile_allocation);
    // One tile to a supertile, as this is small enough to stay under the 256 allowed.
    memory.bytes(&[OP_MULTICORE_RENDERING_SUPERTILE_CFG, 0, 0, TILES_X as u8, TILES_Y as u8]);
    memory.u32(TILES_X as u32 | (TILES_Y as u32) << 12);

    // Clears the tile buffer, along with a dummy store that works around a hardware race.
    memory.u8(OP_TILE_COORDINATES).bytes(&[0, 0, 0]);
    memory.u8(OP_END_OF_LOADS);
    write_store(memory, BUFFER_NONE, 0, 0, 0);
    memory.bytes(&[OP_CLEAR_TILE_BUFFERS, 0b11]);
    memory.u8(OP_END_OF_TILE_MARKER);
    memory.u8(OP_TILE_COORDINATES).bytes(&[0, 0, 0]);
    memory.u8(OP_END_OF_LOADS);
    write_store(memory, BUFFER_NONE, 0, 0, 0);
    memory.u8(OP_END_OF_TILE_MARKER);
    memory.u8(OP_FLUSH_VCD_CACHE);

    memory.u8(OP_START_ADDRESS_OF_GENERIC_TILE_LIST).u32(tile_list_start).u32(tile_list_end);
    for y in 0..TILES_Y as u8 {
        for x in 0..TILES_X as u8 {
            memory.bytes(&[OP_SUPERTILE_COORDINATES, x, y]);
        }
    }
    memory.u8(OP_END_OF_RENDERING);
    let render_end = memory.next as u32;

    Lists { bin_start, bin_end, render_start, render_end, tile_allocation, tile_state, positions, colors }
}
