#include "gic.h"
#include "mem.h"
#include "uart.h"

void enableIRQ(char irq) {
  uart_puts("enableIRQ\r\n");

  uint32_t* addr = (uint32_t*)GICD_IRQ_SET_ENABLE + (irq / 32);
  uint32_t word = 1 << (irq % 32);

  write(addr, word);
}

void routeIRQtoCPUS(char irq, char cpus) {
  uart_puts("routeIRQtoCPUS\r\n");

  uint32_t* addr = (uint32_t*)GICD_IRQ_PROCESSOR_TARGETS + (irq / 4);

  uint32_t word = read(addr);
  char offset = irq % 4;
  word &= ~(0xff << (offset * 8)); // Clear bits
  word |= cpus << (offset * 8); // Set bits

  write(addr, word);
}
