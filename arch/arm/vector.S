#include "mode.h"

.section ".text.exception_vector"

#define print_hex \
	ldr r1, =uart_putc; \
	bl word_to_hex

#define print(str) \
	adr r0, str; \
	bl uart_puts

#define print_nl print(new_line_str)

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
	b reset_exception                 // +0
	b undefined_instruction_exception // +4
	b software_interrupt_exception    // +8
	b instruction_abort_exception     // +12
	b data_abort_exception            // +16
	b hypervisor_call_exception       // +20
	b irq_exception                   // +24
	b fiq_exception                   // +28

GENERIC_EXCEPTION reset, "RESET"

undefined_instruction_exception:
	push {lr}

	print(undefined_instruction_str)

	ldr r0, [sp]
	sub r0, #8
	print_hex
	print_nl

	pop {lr}
	eret
undefined_instruction_str:
	.asciz "== UNDEFINED INSTRUCTION ==\r\n"
	.align(2)

software_interrupt_exception:
	push {lr}

	print_hex
	print_nl

	print(software_interrupt_str)

	ldr r0, [sp]
	sub r0, #4
	print_hex
	print_nl

	pop {lr}
	eret
software_interrupt_str:
	.asciz "== SOFTWARE INTERRUPT ==\r\n"
	.align(2)

GENERIC_EXCEPTION instruction_abort, "INSTRUCTION ABORT"

data_abort_exception:
	push {r0, r1, lr}

	print(data_abort_str)

	ldr r0, [sp, #8]
	sub r0, #8
	print_hex
	print_nl

	pop {r0, r1, lr}

	@ eret
	b input_loop
data_abort_str:
  .asciz "== DATA ABORT ==\r\n"
  .align(2)

GENERIC_EXCEPTION hypervisor_call, "HYPERVISOR CALL"

irq_exception:
	push {r0-r12, lr}

	// Print SPSR
	// mrs r0, spsr
	// ldr r1, =uart_putc
	// bl word_to_hex
	// adr r0, new_line_str
	// bl uart_puts

	print(irq_str)

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

	print(end_irq_str)

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
