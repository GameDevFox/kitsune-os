.section ".text.exception_vector"
	b reset_exception
	b undefined_instruction_exception
	b software_interrupt_exception
	b instruction_abort_exception
	b data_abort_exception
	b hypervisor_call_exception
	b irq_exception
	b fiq_exception

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
	.asciz "== Undefined  Instruction ==\r\n"
	.align(2)

software_interrupt_exception:
	mov sp, #0x100000
	push {lr}

	adr r0, software_interrupt_str
	bl uart_puts

	pop {lr}
	eret
software_interrupt_str:
	.asciz "== SOFTWARE INTERRUPT ==\r\n"
	.align(2)

irq_exception:
	mov sp, #0x100000
	push {lr}

	adr r0, irq_str
	bl uart_puts

	pop {lr}
	eret
irq_str:
	.asciz "== IRQ ==\r\n"
	.align(2)

fiq_exception:
	mov sp, #0x100000
	push {lr}

	adr r0, fiq_str
	bl uart_puts

	pop {lr}
	eret
fiq_str:
	.asciz "== FIQ ==\r\n"
	.align(2)

new_line_str:
	.asciz "\r\n"
	.align(2)
