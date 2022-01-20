#include <stddef.h>

#include "convert.h"
#include "uart.h"

size_t binary_entry = 0xffff5500;

char getb() {
  char input = uart_getc();
  return char_as_hex(input);
}

size_t uart_getw() {
  size_t word = 0;
  word |= (uart_getc() << 0);
  word |= (uart_getc() << 8);
  word |= (uart_getc() << 16);
  word |= (uart_getc() << 24);

  return word;
}

void binary_entry_mode() {
  uart_puts("== ENTRY MODE ==\r\n");

  char input = 0;

  char running = 1;
  while(running) {
    word_to_hex(binary_entry, uart_putc);
    uart_puts("\r\n");

    input = uart_getc();

    if(input == 0x0d) // ENTER
      break;

    if(input == 0x7f) // DEL
      binary_entry = (binary_entry >> 4);
    else {
      char value = char_as_hex(input);

      if(value != 0xff)
        binary_entry = (binary_entry << 4) | value;
      else {
        uart_puts("Oops...\r\n");
        continue;
      }
    }
  }

  uart_puts("== INPUT MODE ==\r\n");
}
