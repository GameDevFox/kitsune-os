// AArch32 mode

// To keep this in the first portion of the binary.
.section ".text.boot"

// Make _start global.
.globl _start

        .org 0x8000
// Entry point for the kernel.
// r15 -> should begin execution at 0x8000.
// r0 -> 0x00000000
// r1 -> 0x00000C42 - machine id
// r2 -> 0x00000100 - start of ATAGS
// preserve these registers as argument for kernel_main
_start:
	mrc p15, 0, r0, c9, c12, 0
	orr r0, r0, #0x1 // Enable PMCR.E
	mcr p15, 0, r0, c9, c12, 0

	mrc p15, 0, r0, c9, c12, 1
	orr r0, r0, #0x80000000 // Enable PMCNTENSET.C
	mcr p15, 0, r0, c9, c12, 1

	mrc p15, 0, r0, c9, c13, 0
	mrc p15, 0, r1, c9, c13, 0
	mrc p15, 0, r2, c9, c13, 0
	mrc p15, 0, r3, c9, c13, 0
	mrc p15, 0, r4, c9, c13, 0
	mrc p15, 0, r5, c9, c13, 0
	mrc p15, 0, r6, c9, c13, 0
	mrc p15, 0, r7, c9, c13, 0

	mov r0, #0

	// Shut off extra cores
	mrc p15, 0, r5, c0, c0, 5
	and r5, r5, #3
	cmp r5, #0
	bne halt

	// Setup the stack.
	ldr r5, =_start
	mov sp, r5

	// Clear out bss.
	ldr r4, =__bss_start
	ldr r9, =__bss_end
	mov r5, #0
	mov r6, #0
	mov r7, #0
	mov r8, #0
	b       2f

1:
	// store multiple at r4.
	stmia r4!, {r5-r8}

	// If we are still below bss_end, loop.
2:
	cmp r4, r9
	blo 1b

	// Call kernel_main
	ldr r3, =kernel_main
	blx r3

	// halt
halt:
	wfe
	b halt
