// Must match the layout of TrapFrame in exception.rs.
.equ FRAME_SIZE, 272
.equ FRAME_X30_ELR, 240
.equ FRAME_SPSR, 256

// Each vector table entry is 0x80 bytes, and the table must be 2 KB aligned.
.equ VECTOR_ALIGNMENT, 0x80
.equ TABLE_ALIGNMENT, 0x800

.section ".text.vectors", "ax"

.macro vector kind
    .balign VECTOR_ALIGNMENT
    sub     sp, sp, #FRAME_SIZE
    stp     x0, x1, [sp, #0]
    mov     x0, #\kind
    b       exception_common
.endm

// `kind` is the entry's index in this table, which exception.rs decodes.
.balign TABLE_ALIGNMENT
.globl exception_vectors
exception_vectors:
    vector 0    // Current EL using SP_EL0
    vector 1
    vector 2
    vector 3
    vector 4    // Current EL using SP_ELx: the kernel itself
    vector 5
    vector 6
    vector 7
    vector 8    // Lower EL, AArch64
    vector 9
    vector 10
    vector 11
    vector 12   // Lower EL, AArch32
    vector 13
    vector 14
    vector 15

// On entry x0 is the kind, and the old x0 and x1 are already in the frame.
exception_common:
    stp     x2, x3, [sp, #16]
    stp     x4, x5, [sp, #32]
    stp     x6, x7, [sp, #48]
    stp     x8, x9, [sp, #64]
    stp     x10, x11, [sp, #80]
    stp     x12, x13, [sp, #96]
    stp     x14, x15, [sp, #112]
    stp     x16, x17, [sp, #128]
    stp     x18, x19, [sp, #144]
    stp     x20, x21, [sp, #160]
    stp     x22, x23, [sp, #176]
    stp     x24, x25, [sp, #192]
    stp     x26, x27, [sp, #208]
    stp     x28, x29, [sp, #224]
    mrs     x2, elr_el1
    mrs     x3, spsr_el1
    stp     x30, x2, [sp, #FRAME_X30_ELR]
    str     x3, [sp, #FRAME_SPSR]

    mov     x1, sp
    bl      handle_exception

    // The handler may have edited the frame, for example to skip an instruction.
    ldp     x30, x2, [sp, #FRAME_X30_ELR]
    ldr     x3, [sp, #FRAME_SPSR]
    msr     elr_el1, x2
    msr     spsr_el1, x3
    ldp     x2, x3, [sp, #16]
    ldp     x4, x5, [sp, #32]
    ldp     x6, x7, [sp, #48]
    ldp     x8, x9, [sp, #64]
    ldp     x10, x11, [sp, #80]
    ldp     x12, x13, [sp, #96]
    ldp     x14, x15, [sp, #112]
    ldp     x16, x17, [sp, #128]
    ldp     x18, x19, [sp, #144]
    ldp     x20, x21, [sp, #160]
    ldp     x22, x23, [sp, #176]
    ldp     x24, x25, [sp, #192]
    ldp     x26, x27, [sp, #208]
    ldp     x28, x29, [sp, #224]
    ldp     x0, x1, [sp, #0]
    add     sp, sp, #FRAME_SIZE
    eret
