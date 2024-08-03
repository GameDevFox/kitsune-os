#include <stddef.h>

#include "convert.h"
#include "uart.h"
#include "vt.h"

struct VT_Size vt_get_pos() {
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

void vt_set_pos(int column, int row) {
  // "\e[" #row ";" #column "H"
  uart_puts("\e[");
  write_digits(row, uart_putc);
  uart_putc(';');
  write_digits(column, uart_putc);
  uart_putc('H');
}

struct VT_Size vt_get_size() {
  struct VT_Size pos = vt_get_pos();
  struct VT_Size prev;

  uart_puts(VT_SAVE);

  do {
    prev = pos;
    uart_puts(VT_RIGHT(999) VT_DOWN(999));
    pos = vt_get_pos();
  } while(
    pos.row != prev.row ||
    pos.column != prev.column
  );

  uart_puts(VT_LOAD);

  return pos;
}

void vt_fill(char c) {
  struct VT_Size size = vt_get_size();

  uart_puts(VT_SAVE);

  for(size_t row = 1; row <= size.row; row++) {
    vt_set_pos(0, row);

    for(size_t column = 0; column < size.column; column++) {
      uart_putc(c);
    }
  }

  uart_puts(VT_LOAD);
}
