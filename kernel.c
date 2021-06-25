#include <stddef.h>
#include <stdint.h>

#include "kernel.h"
#include "uart.h"

void reset_exception() {
	uart_puts("== RESET EXCEPTION ==\r\n");
}

void bad_instruction_exception() {
	uart_puts("== BAD INSTRUCTION EXCEPTION ==\r\n");
}

void software_interrupt_exception() {
	uart_puts("== SOFTWARE INTERRUPT ==\r\n");
}

void instruction_abort_exception() {
	uart_puts("== INSTRUCTION ABORT EXCEPTION ==\r\n");
}

void data_abort_exception() {
	uart_puts("== DATA ABORT EXCEPTION ==\r\n");
}

void hypervisor_call_exception() {
	uart_puts("== HYPERVISOR CALL EXCEPTION ==\r\n");
}

void irq_exception() {
	uart_puts("== IRQ ==\r\n");
}

void fiq_exception() {
	uart_puts("== FIQ ==\r\n");
}

static char* hexTable = "0123456789abcdef";

void byte_to_hex(unsigned char b, void (*out)(unsigned char)) {
	unsigned char first4 = (b & 0xf0) >> 4;
	unsigned char last4 = b & 0xf;

	unsigned char firstChar = hexTable[first4];
	unsigned char secondChar = hexTable[last4];

	out(firstChar);
	out(secondChar);
}

void word_to_hex(size_t word, void (*out)(unsigned char)) {
	byte_to_hex(word >> 24 & 0xff, out);
	byte_to_hex(word >> 16 & 0xff, out);
	byte_to_hex(word >> 8  & 0xff, out);
	byte_to_hex(word >> 0  & 0xff, out);
}

void printCycleCounterList() {
	uart_puts("CYCLE COUNTER LIST:\r\n");
	size_t prev = 0;
	for(int i=0; i<6; i++) {
		size_t* address = (void*)0x40 + (i * 4);
		printAddress(address, uart_putc);

		size_t current = *address;
		size_t delta = current - prev;
		prev = current;

		if(i != 0) {
			uart_puts(" :: ");
			word_to_hex(delta, uart_putc);
		}

		uart_puts("\r\n");
	}
}

void printParams(uint32_t r0, uint32_t r1, uint32_t atags) {
	uart_puts("r0:    0x");
	word_to_hex(r0, uart_putc);
	uart_puts("\r\n");

	uart_puts("r1:    0x");
	word_to_hex(r1, uart_putc);
	uart_puts("\r\n");

	uart_puts("atags: 0x");
	word_to_hex(atags, uart_putc);
	uart_puts("\r\n");

	uart_puts("\r\n");
}

#define MRC_BASE 0xee100010

size_t generateMRC(
	char coproc, char opc1, char Rt,
	char CRn, char CRm, char opc2
) {
	size_t result = MRC_BASE;

	result |= coproc << 8;
	result |= opc1   << 21;
	result |= Rt     << 12;
	result |= CRn    << 16;
	result |= CRm    << 0;
	result |= opc2   << 5;

	return result;
}

char charAsHex(char input) {
	char binary;
	if(input >= '0' && input <= '9') // Number
		binary = (input - 0x30);
	else if(input >= 'a' && input <= 'f') // Letter
		binary = (input - 0x57);
	else
		binary = 0xff;

	return binary;
}

char getb() {
	char input = uart_getc();
	return charAsHex(input);
}

size_t binaryEntry;

void binaryEntryMode() {
	uart_puts("== ENTRY MODE ==\r\n");

	char input = 0;

	char running = 1;
	while(running) {
		word_to_hex(binaryEntry, uart_putc);
		uart_puts("\r\n");

		input = uart_getc();

		if(input == 0x0d) // ENTER
			break;

		if(input == 0x7f) // DEL
			binaryEntry = (binaryEntry >> 4);
		else {
			char value = charAsHex(input);

			if(value != 0xff)
				binaryEntry = (binaryEntry << 4) | value;
			else {
				uart_puts("Oops...\r\n");
				continue;
			}
		}
	}

	uart_puts("== INPUT MODE ==\r\n");
}

#define SWI(value) "swi #" #value

char isPollingEnabled = 1;

void input_loop() {
	char input = 0;

	while (1) {
		if(isPollingEnabled)
			input = uart_getc();

		switch(input) {
			case '=':
				binaryEntryMode();
				break;
			case '~':
				// Generate MRC instuction
				uart_puts("coproc\r\n");
				char coproc = getb();
				uart_puts("opc1\r\n");
				char opc1 = getb();
				// uart_puts("Rt\r\n");
				// char Rt = getb();
				uart_puts("CRn\r\n");
				char CRn = getb();
				uart_puts("CRm\r\n");
				char CRm = getb();
				uart_puts("opc2\r\n");
				char opc2 = getb();

				char Rt = 0;
				binaryEntry = generateMRC(
					// 15, 4, 0, 1, 1, 0
					coproc, opc1, Rt, CRn, CRm, opc2
				);
				word_to_hex(binaryEntry, uart_putc);
				uart_puts("\r\n");
				break;
			case '!':
				// Write instruction to our function
				*(size_t*)(&liveInstructionCall) = binaryEntry;

				// Call function and print result
				size_t result = liveInstructionCall();
				word_to_hex(result, uart_putc);
				uart_puts("\r\n");
				break;

			case 0x0d: // ENTER
				uart_puts("\r\n");
				break;
			case 0x7f: // DEL
				// TODO: Fix this hacky way, there must be a better way
				uart_putc('\b');
				uart_putc(' ');
				uart_putc('\b');
				break;

			case '1':
				uart_puts("Hello, Kitsune!\r\n");
				break;
			case '2':
				asm(SWI(1234));
				break;
			case '3':
				size_t mode = getMode();
				word_to_hex(mode, uart_putc);
				uart_puts("\r\n");
				break;
			case '4':
				perfTest();
				printCycleCounterList();
				uart_puts("Done\r\n");
				uart_puts("\r\n");
				break;
			// case '7':
			// 	size_t iMasks = uart_getc(UART0_IMSC);
			// 	word_to_hex(iMasks, uart_putc);
			// 	uart_puts("\r\n");
			// 	iMasks = iMasks & ~((size_t)0x30);
			// 	uart_putc(UART0_IMSC, iMasks);
			// 	iMasks = uart_getc(UART0_IMSC);
			// 	word_to_hex(iMasks, uart_putc);
			// 	uart_puts("\r\n");
			// 	break;
			case '0':
				isPollingEnabled = 0;
				break;

			default:
				uart_putc(input);
				break;
		}
	}
}

void memoryRangeOut(size_t* start, size_t count, void (*out)(unsigned char)) {
	size_t* end = start + count;

	for(size_t* i = start; i < end; i++) {
		printAddress(i, out);
		out('\r');
		out('\n');
	}

	out('\r');
	out('\n');
}

struct deviceTreeHeader {
	uint32_t magic;
	uint32_t totalSize;

	uint32_t structureOffset;
	uint32_t stringsOffset;
	uint32_t memoryReservationOffset;

	uint32_t version;
	uint32_t lastCompatibleVersion;

	uint32_t bootCpuPhysicalId;

	uint32_t stringsSize;
	uint32_t structureSize;
};

#if defined(__cplusplus)
extern "C" /* Use C linkage for kernel_main. */
#endif

#ifdef AARCH64
// arguments for AArch64
void kernel_main(uint64_t dtb_ptr32, uint64_t x1, uint64_t x2, uint64_t x3)
#else
// arguments for AArch32
void kernel_main(uint32_t r0, uint32_t r1, uint32_t atags)
#endif
{
	// Initialize UART for Raspberry Pi
	uart_init(RASPI_VERSION);

	uart_puts("Hello, Kitsune!\r\n");
	uart_puts("\r\n");

	printParams(r0, r1, atags);

	// uart_puts("DEVICE TREE:\r\n");
	// memoryRangeOut((size_t*)atags, 32, uart_putc);

	// printCycleCounterList();
	// uart_puts("\r\n");

	uart_puts("READY\r\n");

	input_loop();
}
