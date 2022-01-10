#include <stddef.h>

#include "convert.h"
#include "uart.h"
#include "vt.h"

struct VT_Size VT_getPos() {
  struct VT_Size pos = { 0, 0 };

  uart_puts(VT_GET_POS);

  char c;
  do {
    c = uart_getc();
  } while(c != '[');
  c = uart_getc();

  while(c != ';') {
    read_digit(&pos.row, c);
    c = uart_getc();
  }
  c = uart_getc();

  while(c != 'R') {
    read_digit(&pos.column, c);
    c = uart_getc();
  }

  return pos;
}

void VT_setPos(int column, int row) {
  // "\e[" #row ";" #column "H"
  uart_puts("\e[");
  write_digits(row, uart_putc);
  uart_putc(';');
  write_digits(column, uart_putc);
  uart_putc('H');
}

struct VT_Size VT_getSize() {
  struct VT_Size pos = VT_getPos();
  struct VT_Size prev;

  uart_puts(VT_SAVE);

  do {
    prev = pos;
    uart_puts(VT_RIGHT(999) VT_DOWN(999));
    pos = VT_getPos();
  } while(
    pos.row != prev.row ||
    pos.column != prev.column
  );

  uart_puts(VT_LOAD);

  return pos;
}

void VT_fill(char c) {
  struct VT_Size size = VT_getSize();

  uart_puts(VT_SAVE);

  for(size_t row = 1; row <= size.row; row++) {
    VT_setPos(0, row);

    for(size_t column = 0; column < size.column; column++) {
      uart_putc(c);
    }
  }

  uart_puts(VT_LOAD);
}
