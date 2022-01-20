#include "mode.h"

.section ".text.exception_vector"

.macro GENERIC_EXCEPTION name, str
\name\()_exception:
	push {lr}
	mov sp, #0x100000

	adr r0, \name\()_str
	bl uart_puts

	pop {lr}
	eret
\name\()_str:
  .asciz "== \str ==\r\n"
  .align(2)
.endm

.globl exception_vector
exception_vector:
	b reset_exception
	b undefined_instruction_exception
	b software_interrupt_exception
	b instruction_abort_exception
	b data_abort_exception
	b hypervisor_call_exception
	b irq_exception
	b fiq_exception

GENERIC_EXCEPTION reset, "RESET"

// High
// (8)  16M
// 12   1M
// Mid
// (16) 64K
// 20   16K
// 4K Lower

software_interrupt_exception:
	push {lr}

	mrc p15, 0, r0, c1, c1, 0 // Read SCR into r0
	ldr r1, =uart_putc
	bl word_to_hex
	adr r0, new_line_str
	bl uart_puts

	adr r0, software_interrupt_str
	bl uart_puts

	ldr r0, [sp]
	sub r0, #4
	ldr r1, =uart_putc
	bl word_to_hex

	adr r0, new_line_str
	bl uart_puts

	pop {lr}
	eret
software_interrupt_str:
	.asciz "== SOFTWARE INTERRUPT ==\r\n"
	.align(2)

undefined_instruction_exception:
	mov sp, #0x100000
	push {lr}

	adr r0, undefined_instruction_str
	bl uart_puts

	ldr r0, [sp]
	sub r0, #8
	ldr r1, =uart_putc
	bl word_to_hex

	adr r0, new_line_str
	bl uart_puts

	pop {lr}
	eret
undefined_instruction_str:
	.asciz "== UNDEFINED INSTRUCTION ==\r\n"
	.align(2)

// GENERIC_EXCEPTION software_interrupt, "SOFTWARE INTERRUPT"
GENERIC_EXCEPTION instruction_abort, "INSTRUCTION ABORT"
GENERIC_EXCEPTION data_abort, "DATA ABORT"
GENERIC_EXCEPTION hypervisor_call, "HYPERVISOR CALL"

irq_exception:
	mov sp, #0x100000
	push {r0-r12, lr}

	// Print SPSR
	// mrs r0, spsr
	// ldr r1, =uart_putc
	// bl word_to_hex
	// adr r0, new_line_str
	// bl uart_puts

	adr r0, irq_str // == IRQ ==
	bl uart_puts

	// Get Interrupt ID
	mov r1, #0x200c
	movt r1, #0xff84
	ldr r0, [r1]

	mov r4, r0

	// Clear Timer 2
	mov r0, #1
	bl clear_timer_compare

	// Set End of Interrupt Register
	mov r1, #0x2010
	movt r1, #0xff84
	mov r0, r4
	str r0, [r1]

	adr r0, end_irq_str // == END IRQ ==
	bl uart_puts

	pop {r0-r12, lr}
	subs pc, lr, #4
irq_str:
  .asciz "== IRQ ==\r\n"
end_irq_str:
	.asciz "== END IRQ ==\r\n"
  .align(2)

GENERIC_EXCEPTION fiq, "FIQ"

new_line_str:
	.asciz "\r\n"
	.align(2)
