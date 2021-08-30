.section ".text.exception_vector"

.macro GENERIC_EXCEPTION name, str
\name\()_exception:
	mov sp, #0x100000
	push {lr}

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

undefined_instruction_exception:
	mov sp, #0x100000
	push {lr}

	adr r0, undefined_instruction_str
	bl uart_puts

	ldr r0, [sp]
	sub r0, #4
	ldr r1, =uart_putc
	bl word_to_hex

	adr r0, new_line_str
	bl uart_puts

	pop {lr}
	eret
undefined_instruction_str:
	.asciz "== UNDEFINED INSTRUCTION ==\r\n"
	.align(2)

new_line_str:
	.asciz "\r\n"
	.align(2)

GENERIC_EXCEPTION software_interrupt, "SOFTWARE INTERRUPT"
GENERIC_EXCEPTION instruction_abort, "INSTRUCTION ABORT"
GENERIC_EXCEPTION data_abort, "DATA ABORT"
GENERIC_EXCEPTION hypervisor_call, "HYPERVISOR CALL"
GENERIC_EXCEPTION irq, "IRQ"
GENERIC_EXCEPTION fiq, "FIQ"
