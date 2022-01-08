#include <stddef.h>

#include "convert.h"
#include "uart.h"

size_t binaryEntry = 0xffff5500;

char getb() {
	char input = uart_getc();
	return charAsHex(input);
}

size_t uart_getw() {
	size_t word = 0;
	word |= (uart_getc() << 0);
	word |= (uart_getc() << 8);
	word |= (uart_getc() << 16);
	word |= (uart_getc() << 24);

	return word;
}

void binaryEntryMode() {
	uart_puts("== ENTRY MODE ==\r\n");

	char input = 0;

	char running = 1;
	while(running) {
		wordToHex(binaryEntry, uart_putc);
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
