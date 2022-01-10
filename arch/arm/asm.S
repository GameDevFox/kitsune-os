#include "coproc.h"
#include "mode.h"

.section ".text"

.globl perfTest
perfTest:
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

.globl getPerformanceCounter
getPerformanceCounter:
	mrc PMCCNTR(r0)
	bx lr

.globl getCPSR
getCPSR:
	mrs r0, cpsr
	bx lr

.globl getMode
getMode:
	mrs r0, cpsr
	and r0, r0, #CPSR_MODE_MASK
	bx lr

.globl setMode
setMode:
	smc #0
	msr sp_hyp, sp
	msr elr_hyp, lr
	msr cpsr_c, r0
	bx lr

.global toggleIRQs
toggleIRQs:
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

// Allowcate empty space for inital register values
.globl initialRegisterValues
initialRegisterValues:
.space 16 * 4

