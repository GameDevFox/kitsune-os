#include "mode.h"

// To keep this in the first portion of the binary.
.section ".text.boot"

// Entry point for the kernel.
// r15 -> should begin execution at 0x8000.
// r0 -> 0x00000000
// r1 -> 0x00000C42 - machine id
// r2 -> 0x00000100 - start of ATAGS
// preserve these registers as argument for kernel_main
.globl _start
_start:
	// Setup the stack and save r0, r1, r2
	ldr sp, =_start
	push {r0-r2}

disable_extra_cores:
	// Shut off extra cores
	mrc p15, 0, r5, c0, c0, 5
	and r5, r5, #3
	cmp r5, #0
	bne halt

// misc:
// 	mov	r7, #0
// 	mcr	p15, 4, r7, c1, c1, 0	// HCR

check_for_hyp_mode:
	bl getMode
	cmp r0, #HYP_MODE
	bleq switchHypToSvc

// enable_high_exception_vectors:
// 	mrc p15, 0, r0, c1, c0, 0
// 	orr r0, #0x00002000
// 	mcr p15, 0, r0, c1, c0, 0

// 	mov r2, #0xff000000
// 	orr r2, #0x00ff0000

set_vbar:
	// Set VBAR to jump table
	mov r0, #0x2000
	mcr p15, 0, r0, c12, c0, 0

// load_jump_table:
// 	adr r1, jump_table
// 	mov r2, #0
// 	adr r4, jump_table_addresses_end
// 1:
// 	ldr r0, [r1]
// 	str r0, [r2]
// 	add r1, r1, #4
// 	add r2, r2, #4
// 	cmp r1, r4
// 	blt 1b

enable_cycle_counter:
	mrc p15, 0, r0, c9, c12, 1
	orr r0, r0, #0x80000000 // Enable PMCNTENSET.C
	mcr p15, 0, r0, c9, c12, 1

	mrc p15, 0, r0, c9, c12, 0
	orr r0, r0, #0x1 // Enable PMCR.E
	mcr p15, 0, r0, c9, c12, 0

	mov r0, #0x1f;
	mcr p15, 0, r0, c9, c12, 5 // Set PMSELR.SEL to CCNT (#31)

	// Don't need this anymore since we switched
	// out of HYP (PL2) to SVC (PL1)
	// mrc p15, 0, r0, c9, c13, 1
	// orr r0, r0, #0x8000000 // Write PMXEVTYPER.NSH
	// mcr p15, 0, r0, c9, c13, 1

	// Clear out bss.
		// TODO: Is this needed in Kitsune since
		// we plan on having abstracted memory?
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

	bl perfTest

enter_kernel:
	// Pop the three registers (r0, r1, r2) we pushed earlier
	pop {r0, r1, r2}

	// Call kernel_main
	ldr r3, =kernel_main
	blx r3

halt:
	wfe
	b halt

.globl perfTest
perfTest:
	push {r0-r7}

	// Get cycle count register
	mrc p15, 0, r0, c9, c13, 0 // Read PMCCNTR
	mrc p15, 0, r1, c9, c13, 0
	mrc p15, 0, r2, c9, c13, 0
	mrc p15, 0, r3, c9, c13, 0
	mrc p15, 0, r4, c9, c13, 0
	mrc p15, 0, r5, c9, c13, 0
	// mrc p15, 0, r6, c9, c13, 0
	// mrc p15, 0, r7, c9, c13, 0

	mov r12, #0x40

	str r0, [r12, #0x00]
	str r1, [r12, #0x04]
	str r2, [r12, #0x08]
	str r3, [r12, #0x0c]
	str r4, [r12, #0x10]
	str r5, [r12, #0x14]
	// str r6, [r12, #0x18]
	// str r7, [r12, #0x1c]

	pop {r0-r7}

	bx lr

.globl getMode
getMode:
	mrs r0, cpsr
	and r0, r0, #CPSR_MODE_MASK
	bx lr

.globl switchHypToSvc
switchHypToSvc:
	// Update SPSR_hyp
	mrs r0, CPSR
	bic r0, #CPSR_MODE_MASK
	orr r0, #SVC_MODE
	msr SPSR_hyp, r0

	// Update ELR_hyp
	adr r0, 1f
	msr ELR_hyp, r0

	// Copy sp and lr to *_svc because
	// they are banked registers in SVC
	msr LR_svc, lr
	msr SP_svc, sp

	eret
1:
	bx lr

.globl liveFn
liveFn:
	push {r1-r12}
.globl liveInstruction
liveInstruction:
	.word 0xee100f10
	pop {r1-r12}
	bx lr

// jump_table:
// 	ldr pc, [pc, #0x18]
// 	ldr pc, [pc, #0x18]
// 	ldr pc, [pc, #0x18]
// 	ldr pc, [pc, #0x18]
// 	ldr pc, [pc, #0x18]
// 	ldr pc, [pc, #0x18]
// 	ldr pc, [pc, #0x18]
// 	ldr pc, [pc, #0x18]
// jump_table_addresses:
// 	.word reset_exception
// 	.word undefined_instruction_exception_stub
// 	.word software_interrupt_exception
// 	.word instruction_abort_exception
// 	.word data_abort_exception
// 	.word hypervisor_call_exception
// 	.word irq_exception
// 	.word fiq_exception
// jump_table_addresses_end:

// Allowcate empty space for inital register values
.globl initialRegisterValues
initialRegisterValues:
.space 16 * 4

