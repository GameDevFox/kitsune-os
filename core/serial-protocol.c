#include "./uart.h"
#include "./serial-protocol.h"

void sp_start() {
  uart_putc(SP_START);
}

void sp_end() {
  uart_putc(SP_ESCAPE);
  uart_putc(SP_END);
}

void sp_target(unsigned char target) {
  uart_putc(SP_ESCAPE);
  uart_putc(SP_TARGET);
  uart_putc(target);
}

void sp_putc(unsigned char c) {
  switch(c) {
    case SP_START:
      uart_putc(SP_ESCAPE);
      uart_putc(SP_LITERAL_START);
      break;
    case SP_ESCAPE:
      uart_putc(SP_ESCAPE);
      uart_putc(SP_LITERAL_ESCAPE);
      break;
    default:
      uart_putc(c);
      break;
  }
}

void sp_putw(uint32_t word) {
  sp_putc(word >> 0 & 0xff);
  sp_putc(word >> 8 & 0xff);
  sp_putc(word >> 16 & 0xff);
  sp_putc(word >> 24 & 0xff);
}

void sp_puts(char* str) {
  for(uint32_t i = 0; str[i] != '\0'; i++)
      sp_putc(str[i]);
}

void sp_write(char* str, uint32_t count) {
  for(uint32_t i = 0; i < count; i++)
      sp_putc(str[i]);
}

void sp_log(char* str) {
  sp_start();
  sp_puts(str);
  sp_end();
}
