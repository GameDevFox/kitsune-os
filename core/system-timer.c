#include <stdint.h>

#include "mem.h"
#include "system-timer.h"
#include "uart.h"

uint32_t readTimer() {
  return read(STIMER_LO_COUNTER);
}

uint32_t writeTimerCompare(char timer, uint32_t time) {
  uart_puts("writeTimerCompare\r\n");

  uint32_t* addr = (uint32_t*)STIMER_COMPARE_0 + timer;

  word_to_hex(addr, uart_putc);
  uart_puts("\r\n");
  word_to_hex(time, uart_putc);
  uart_puts("\r\n");

  return write(addr, time);
}
