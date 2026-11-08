.equ CURRENT_EL_3, 0b1100
.equ CURRENT_EL_2, 0b1000

// SCR_EL3: lower levels are non-secure (bit 0) and AArch64 (bit 10). Bits 4 and
// 5 are reserved and must be 1.
.equ SCR_EL3_VALUE, (1 << 10) | (0b11 << 4) | (1 << 0)
// HCR_EL2: EL1 is AArch64.
.equ HCR_EL2_VALUE, (1 << 31)
// SCTLR_EL1: the reserved-as-1 bits only, so the MMU and caches are off.
.equ SCTLR_EL1_VALUE, 0x30D00800

// SCTLR_EL1 bit 0: the MMU. Bit 2: the data cache.
.equ SCTLR_MMU, (1 << 0)
.equ SCTLR_DATA_CACHE, (1 << 2)
// SCTLR_EL1 bit 12: the instruction cache. It works with the MMU off.
.equ SCTLR_INSTRUCTION_CACHE, (1 << 12)

// How much memory after _start can hold kernel code: link.ld's limit on its size.
.equ KERNEL_ROOM_HIGH_BITS, 0x18
.equ CACHE_LINE_BYTES, 64

// DAIF: debug, SError, IRQ and FIQ, all four masked by `daifset`.
.equ DAIF_ALL, 0b1111

// SPSR: all interrupts masked (bits 6-9), returning to ELx using SP_ELx.
.equ SPSR_MASKED, (0b1111 << 6)
.equ SPSR_EL2H, SPSR_MASKED | 0b1001
.equ SPSR_EL1H, SPSR_MASKED | 0b0101

.section ".text.boot", "ax"

// x0 holds the firmware's device tree pointer, so nothing here may change it before
// it's saved, in `in_el1`.
.globl _start
_start:
    // After `cargo pi`, an interrupt the old kernel enabled could arrive while this one
    // is being set up, and jump into code that has been replaced. So mask them all, and
    // stop the timer the old kernel ticked with, before anything else.
    msr     daifset, #DAIF_ALL
    msr     cntv_ctl_el0, xzr
    isb

    // `cargo pi` writes a new kernel over the old one without the caches noticing, so
    // they can still hold the old code, in L2 too. This must run first, and must not
    // read any literal, as a stale copy of it would look in the wrong place. A stale
    // copy still does the job, as any version of this clears the same memory.
    adr     x1, _start
    movz    x2, #KERNEL_ROOM_HIGH_BITS, lsl #16
    add     x2, x1, x2
forget_stale_code:
    dc      civac, x1
    add     x1, x1, #CACHE_LINE_BYTES
    cmp     x1, x2
    b.lo    forget_stale_code
    dsb     sy
    ic      iallu
    dsb     sy
    isb

    // After `cargo pi`, the MMU is still on, using tables in .bss, which gets zeroed
    // below. Memory is mapped 1:1, so this doesn't move the code running here.
    mrs     x1, sctlr_el1
    bic     x1, x1, #SCTLR_MMU
    bic     x1, x1, #SCTLR_DATA_CACHE
    msr     sctlr_el1, x1
    isb

    // The old kernel can leave dirty lines anywhere, such as its stack, which would
    // overwrite this kernel's memory when they get evicted. So clean and invalidate
    // every line of every data cache, by set and way. x0 must survive this.
    mrs     x1, clidr_el1
    ubfx    x2, x1, #24, #3         // The levels to clean.
    lsl     x2, x2, #1              // Doubled, as `csselr_el1` and `dc cisw` want them.
    mov     x3, #0                  // The current level, doubled.
clean_next_level:
    cmp     x3, x2
    b.hs    cleaned_all_levels
    add     x4, x3, x3, lsr #1      // This level's field in CLIDR, 3 bits each.
    lsr     x4, x1, x4
    and     x4, x4, #7
    cmp     x4, #2                  // 2 or more means it holds data.
    b.lo    clean_level_done
    msr     csselr_el1, x3
    isb
    mrs     x4, ccsidr_el1
    and     x5, x4, #7
    add     x5, x5, #4              // Log2 of the line size, for the set's position.
    ubfx    x6, x4, #3, #10         // The highest way.
    clz     w7, w6                  // The way's position, at the top of the word.
    ubfx    x8, x4, #13, #15        // The highest set.
clean_next_set:
    mov     x9, x6
clean_next_way:
    lsl     x10, x9, x7
    lsl     x11, x8, x5
    orr     x10, x10, x11
    orr     x10, x10, x3
    dc      cisw, x10
    subs    x9, x9, #1
    b.hs    clean_next_way
    subs    x8, x8, #1
    b.hs    clean_next_set
clean_level_done:
    add     x3, x3, #2
    b       clean_next_level
cleaned_all_levels:
    dsb     sy
    ic      iallu
    dsb     sy
    isb

    // Only core 0 boots; the others wait here.
    mrs     x1, mpidr_el1
    and     x1, x1, #0xff
    cbnz    x1, park

    // Descend to EL1 from wherever we were started.
    mrs     x1, CurrentEL
    cmp     x1, #CURRENT_EL_3
    b.eq    from_el3
    cmp     x1, #CURRENT_EL_2
    b.eq    from_el2
    b       in_el1

from_el3:
    ldr     x1, =SCR_EL3_VALUE
    msr     scr_el3, x1
    ldr     x1, =SPSR_EL2H
    msr     spsr_el3, x1
    adr     x1, from_el2
    msr     elr_el3, x1
    eret

from_el2:
    ldr     x1, =HCR_EL2_VALUE
    msr     hcr_el2, x1
    ldr     x1, =SCTLR_EL1_VALUE
    msr     sctlr_el1, x1
    ldr     x1, =SPSR_EL1H
    msr     spsr_el2, x1
    adr     x1, in_el1
    msr     elr_el2, x1
    eret

in_el1:
    // Save the firmware's device tree pointer where later boots can still find it.
    // `cargo pi` starts the kernel with x0 at 0, so that keeps the saved one.
    cbz     x0, device_tree_saved
    ldr     x1, =__device_tree_slot
    str     x0, [x1]
device_tree_saved:

    // The stack grows down from the symbol the board's linker script provides.
    ldr     x1, =__stack_top
    mov     sp, x1

    ldr     x1, =exception_vectors
    msr     vbar_el1, x1
    isb

    mrs     x1, sctlr_el1
    orr     x1, x1, #SCTLR_INSTRUCTION_CACHE
    msr     sctlr_el1, x1
    isb

    // Zero .bss.
    ldr     x1, =__bss_start
    ldr     x2, =__bss_end
clear_bss:
    cmp     x1, x2
    b.hs    enter_kernel
    str     xzr, [x1], #8
    b       clear_bss

enter_kernel:
    bl      kernel_main

park:
    wfe
    b       park
