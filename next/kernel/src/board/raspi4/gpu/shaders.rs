//! Shader code for the GPU, as there's no assembler here yet.

// These shaders are the MIT-licensed example's, as compiled by Mesa. The comments are
// what its disassembler prints.

/// `out[0..4] = varying[0..4] * payload_w + r5`: passes the vertex color through.
pub const FRAGMENT_SHADER: [u64; 12] = [
    0x3D10_3186_BB80_0000, // nop ; nop ; ldvary.r0
    0x5400_3046_BBC0_0000, // nop ; fmul r1, r0, rf0
    0x3D10_A186_0582_9000, // fadd rf6, r1, r5 ; nop ; ldvary.r2
    0x5400_30C6_BBC8_0000, // nop ; fmul r3, r2, rf0
    0x3D11_2185_0582_B000, // fadd rf5, r3, r5 ; nop ; ldvary.r4
    0x5400_3006_BBD0_0000, // nop ; fmul r0, r4, rf0
    0x3D30_6184_0582_8000, // fadd rf4, r0, r5 ; nop ; thrsw ; ldvary.r1
    0x5420_3086_BBC4_0000, // nop ; fmul r2, r1, rf0 ; thrsw
    0x3C00_2183_0582_A000, // fadd rf3, r2, r5 ; nop
    0x3C20_3187_3583_E185, // vfpack tlb, rf6, rf5 ; nop ; thrsw
    0x3C00_3187_3583_E103, // vfpack tlb, rf4, rf3 ; nop
    0x3C00_3186_BB80_0000, // nop ; nop
];

/// Takes a position and a color, and writes the screen position, depth, 1/w and color.
pub const VERTEX_SHADER: [u64; 25] = [
    0x3DE0_2187_BC80_7000, // ldvpmv_in rf7, 0 ; nop
    0x3DE0_2189_BC80_7001, // ldvpmv_in rf9, 1 ; nop
    0x3DE0_218A_BC80_7002, // ldvpmv_in rf10, 2 ; nop
    0x3DE0_2183_BC80_7003, // ldvpmv_in rf3, 3 ; nop
    0x3D82_3186_BB80_0000, // nop ; nop ; ldunifrf.rf8
    0x3DE0_2184_BC80_7004, // ldvpmv_in rf4, 4 ; nop
    0x3C40_3186_BB80_0000, // nop ; nop ; ldunif
    0x55E0_2005_BCB8_71C5, // ldvpmv_in rf5, 5 ; fmul r0, rf7, r5
    0x3DE0_2186_BC80_7006, // ldvpmv_in rf6, 6 ; nop
    0x3C40_3181_F581_8000, // ftoin r1, r0 ; nop ; ldunif
    0x3DE0_2180_F883_70C4, // stvpmv 4, rf3 ; nop
    0x5440_3086_BBB8_0240, // nop ; fmul r2, rf9, r5 ; ldunif
    0x3DE0_2180_F883_7105, // stvpmv 5, rf4 ; nop
    0x3DE0_2180_F883_7146, // stvpmv 6, rf5 ; nop
    0x5440_3003_F5B9_A280, // ftoin r3, r2 ; fmul r0, rf10, r5 ; ldunif
    0x3DE0_2180_F883_7187, // stvpmv 7, rf6 ; nop
    0x3DE0_2180_F880_F000, // stvpmv 0, r1 ; nop
    0x3C00_3184_0582_8000, // fadd r4, r0, r5 ; nop
    0x3DE0_2180_F881_F001, // stvpmv 1, r3 ; nop
    0x3DE0_2180_F882_7002, // stvpmv 2, r4 ; nop
    0x3DE0_2180_F883_7203, // stvpmv 3, rf8 ; nop
    0x3C00_3186_BB81_6000, // vpmwt - ; nop
    0x3C20_3186_BB80_0000, // nop ; nop ; thrsw
    0x3C00_3186_BB80_0000, // nop ; nop
    0x3C00_3186_BB80_0000, // nop ; nop
];

/// The part of the vertex shader that binning needs: just where things land.
pub const COORDINATE_SHADER: [u64; 18] = [
    0x3DE0_2184_BC80_7000, // ldvpmv_in rf4, 0 ; nop
    0x3DE0_2185_BC80_7001, // ldvpmv_in rf5, 1 ; nop
    0x3DE0_2183_BC80_7002, // ldvpmv_in rf3, 2 ; nop
    0x3DE0_2180_F883_7100, // stvpmv 0, rf4 ; nop
    0x3DE0_2180_F883_7141, // stvpmv 1, rf5 ; nop
    0x3C40_3186_BB80_0000, // nop ; nop ; ldunif
    0x3DE0_2180_F883_70C2, // stvpmv 2, rf3 ; nop
    0x3DE0_2180_F882_F003, // stvpmv 3, r5 ; nop
    0x3C40_3186_BB80_0000, // nop ; nop ; ldunif
    0x5440_3006_BBB8_0100, // nop ; fmul r0, rf4, r5 ; ldunif
    0x5400_3081_F5B9_8140, // ftoin r1, r0 ; fmul r2, rf5, r5
    0x3DE0_2180_F880_F004, // stvpmv 4, r1 ; nop
    0x3C00_3183_F581_A000, // ftoin r3, r2 ; nop
    0x3DE0_2180_F881_F005, // stvpmv 5, r3 ; nop
    0x3C00_3186_BB81_6000, // vpmwt - ; nop
    0x3C20_3186_BB80_0000, // nop ; nop ; thrsw
    0x3C00_3186_BB80_0000, // nop ; nop
    0x3C00_3186_BB80_0000, // nop ; nop
];
