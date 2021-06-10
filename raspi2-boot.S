// AArch32 mode

#define MODE_MASK 0x1f
#define SVC_MODE  0x13
#define HYP_MODE  0x1a

// To keep this in the first portion of the binary.
.section ".text.boot"

// Make _start global.
.globl _start

// Entry point for the kernel.
// r15 -> should begin execution at 0x8000.
// r0 -> 0x00000000
// r1 -> 0x00000C42 - machine id
// r2 -> 0x00000100 - start of ATAGS
// preserve these registers as argument for kernel_main
_start:

disable_extra_cores:
	// Shut off extra cores
	mrc p15, 0, r5, c0, c0, 5
	and r5, r5, #3
	cmp r5, #0
	bne halt

check_for_hyp_mode:
	bl getMode
	cmp r0, #HYP_MODE
	bleq switchHypToSvc

load_jump_table:
	adr r1, jump_table
	mov r2, #0
	adr r4, jump_table_addresses_end
1:
	ldr r0, [r1]
	str r0, [r2]
	add r1, r1, #4
	add r2, r2, #4
	cmp r1, r4
	blt 1b

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

	bl perfTest

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
	mrc p15, 0, r6, c9, c13, 0
	mrc p15, 0, r7, c9, c13, 0

	mov r12, #0x40

	str r0, [r12, #0x00]
	str r1, [r12, #0x04]
	str r2, [r12, #0x08]
	str r3, [r12, #0x0c]
	str r4, [r12, #0x10]
	str r5, [r12, #0x14]
	str r6, [r12, #0x18]
	str r7, [r12, #0x1c]

	pop {r0-r7}

	bx lr

.globl getMode
getMode:
	mrs r0, cpsr
	and r0, r0, #MODE_MASK
	bx lr

.globl switchHypToSvc
switchHypToSvc:
	// Update SPSR_hyp
	mrs r0, CPSR
	bic r0, #MODE_MASK
	orr r0, #SVC_MODE
	msr SPSR_hyp, r0

	// Update ELR_hyp
	adr r0, fn_end
	msr ELR_hyp, r0

	// Copy lr to LR_svc becaused is banked in SVC
	msr LR_svc, lr

	eret
fn_end:
	bx lr

// 0x10 USR (User)
// 0x11 FIQ
// 0x12 IRQ
// 0x13 SVC (Supervisor)

// 0x16 MON (Monitor)
// 0x17 ABT (Abort)

// 0x1a HYP (Hypervisor)
// 0x1b UND (Undefined)

// 0x1f SYS (System)

jump_table:
	ldr pc, [pc, #0x18]
	ldr pc, [pc, #0x18]
	ldr pc, [pc, #0x18]
	ldr pc, [pc, #0x18]
	ldr pc, [pc, #0x18]
	ldr pc, [pc, #0x18]
	ldr pc, [pc, #0x18]
	ldr pc, [pc, #0x18]
jump_table_addresses:
	.word reset_exception
	.word bad_instruction_exception
	.word software_interrupt_exception
	.word instruction_abort_exception
	.word data_abort_exception
	.word hypervisor_call_exception
	.word irq_exception
	.word fiq_exception
jump_table_addresses_end:
