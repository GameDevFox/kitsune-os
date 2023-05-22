#include "./uart.h"
#include "./section.h"
#include "./serial-protocol.h"

__eden void sp_start() {
  uart_putc(SP_START);
}

__eden void sp_end() {
  uart_putc(SP_ESCAPE);
  uart_putc(SP_END);
}

__eden void sp_target(unsigned char target) {
  uart_putc(SP_ESCAPE);
  uart_putc(SP_TARGET);
  uart_putc(target);
}

__eden void sp_putc(unsigned char c) {
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

__eden void sp_putw(uint32_t word) {
  sp_putc(word >> 0 & 0xff);
  sp_putc(word >> 8 & 0xff);
  sp_putc(word >> 16 & 0xff);
  sp_putc(word >> 24 & 0xff);
}

__eden void sp_puts(char* str) {
  for(uint32_t i = 0; str[i] != '\0'; i++)
      sp_putc(str[i]);
}

__eden void sp_write(char* str, uint32_t count) {
  for(uint32_t i = 0; i < count; i++)
      sp_putc(str[i]);
}

__eden void sp_log(char* str) {
  sp_start();
  sp_puts(str);
  sp_end();
}
