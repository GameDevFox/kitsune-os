#include "coproc.h"
#include "mode.h"

.section ".text"

.global test_bytes
test_bytes:
	// r0 start (inclusive)
	// r1 end (exclusive)

	mov r2, #0xffffffff
1:
	add r2, #1
	strb r2, [r0, #0]
	add r0, #1

	cmp r0, r1
	blo 1b

	bx lr

// 10 words * 4 bytes per word
#define COPY_SIZE 40

// TODO: Optimize and/or clean up this nonsense
.globl copy_bytes
copy_bytes:
	push {r0-r12, lr}

	// r0 - source
	// r1 - dest
	// r2 - length

	// r3 - next (from source)

	// set "next"

	mov r4, #COPY_SIZE
	udiv r3, r2, r4
	mul r3, r4

	add r2, r0 // length -> end

	// If "next" is 0, jump to PHASE 2
	cmp r3, #0
	beq 2f

	//// PHASE 1 - FASTER (multiple words)

	add r3, r0

1:
	// load and store multiple
	ldmia r0!, {r4-r12, lr}
	stmia r1!, {r4-r12, lr}
	cmp r0, r3
	blo 1b

2:
	//// PHASE 2 - SLOWER (single words)

	mov r4, #4 // bytes in a word
	sub r3, r2, r0
	udiv r3, r3, r4
	mul r3, r4

	// If "next" is 0, jump to PHASE 3
	cmp r2, #0
	beq 4f

	// Update "next"
	add r3, r0

3:
	// load and store word
	ldr r4, [r0]
	add r0, #4

	str r4, [r1]
	add r1, #4

	cmp r0, r3
	blo 3b

4:
	// PHASE 3 - SLOWEST (single bytes)

	// load and store byte
	ldrb r4, [r0]
	add r0, #1

	strb r4, [r1]
	add r1, #1

	cmp r0, r2
	blo 4b

	pop {r0-r12, lr}

	bx lr

// 11 words * 4 bytes per word
#define CLEAR_SIZE 44

// TODO: Optimize and/or clean up this nonsense
.globl clear_bytes
clear_bytes:
	push {r0-r12, lr}

	// r0 - start
	// r1 - length / end

	// r2 - next

	// set "next"
	mov r3, #CLEAR_SIZE
	udiv r2, r1, r3
	mul r2, r3

	add r1, r0 // length -> end

	// If "next" is 0, jump to PHASE 2
	cmp r2, #0
	beq 2f

	//// PHASE 1 - FASTER (multiple words)

	add r2, r0

	// setup 11 registers
	mov r3,  #0x00000000
	mov r4,  #0x00000000
	mov r5,  #0x00000000
	mov r6,  #0x00000000

	mov r7,  #0x00000000
	mov r8,  #0x00000000
	mov r9,  #0x00000000
	mov r10, #0x00000000

	mov r11, #0x00000000
	mov r12, #0x00000000
	mov  lr, #0x00000000 // r14

1:
	// store multiple
	stmia r0!, {r3-r12, lr}
	cmp r0, r2
	blo 1b

2:
	//// PHASE 2 - SLOWER (single words)

	mov r3, #4 // bytes in a word
	sub r2, r1, r0
	udiv r2, r2, r3
	mul r2, r3

	mov r3,  #0x00000000

	// If "next" is 0, jump to PHASE 3
	cmp r2, #0
	beq 4f

	// Update "next"
	add r2, r0

3:
	// store word
	str r3, [r0]
	add r0, #4
	cmp r0, r2
	blo 3b

4:
	// PHASE 3 - SLOWEST (single bytes)

	// store byte
	strb r3, [r0]
	add r0, #1
	cmp r0, r1
	blo 4b

	pop {r0-r12, lr}
	bx lr

.globl perf_test
perf_test:
	push {r0-r7}

	// Get cycle count register
	mrc PMCCNTR(r0)
	mrc PMCCNTR(r1)
	mrc PMCCNTR(r2)
	mrc PMCCNTR(r3)
	mrc PMCCNTR(r4)
	mrc PMCCNTR(r5)
	mrc PMCCNTR(r6)
	mrc PMCCNTR(r7)

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

.globl get_pmccntr
get_pmccntr:
	mrc PMCCNTR(r0)
	bx lr

.globl get_performance_counter
get_performance_counter:
	mrc PMCCNTR(r0)
	bx lr

.globl get_cpsr
get_cpsr:
	mrs r0, cpsr
	bx lr

.globl set_cpsr
set_cpsr:
	msr cpsr, r0
	bx lr

.globl get_mode
get_mode:
	mrs r0, cpsr
	and r0, r0, #CPSR_MODE_MASK
	bx lr

.globl set_mode
set_mode:
	smc #0
	msr sp_hyp, sp
	msr elr_hyp, lr
	msr cpsr_c, r0
	bx lr

.global toggle_irqs
toggle_irqs:
	push {lr}
	mrs r0, cpsr
	eor r0, #CPSR_DISABLE_IRQ_FIQ_MASK
	msr cpsr, r0
	pop {pc}

.global enable_cache
enable_cache:
	mrc SCTLR(r0)
	orr r0, #PMCCNTR_C_FLAG
	mcr SCTLR(r0)
	bx lr

new_line_str:
	.asciz "\r\n"
	.align(2)

.globl switch_hyp_to_svc
switch_hyp_to_svc:
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

.globl live_fn
live_fn:
	push {r1-r12}

	// Figure out which of these are actually nessisary
	dmb
	dsb
	isb
.globl live_instruction
live_instruction:
	.word 0xee100f10

	dmb
	dsb
	isb

	pop {r1-r12}
	bx lr

// Allowcate empty space for inital register values
.globl initial_register_values
initial_register_values:
.space 16 * 4

