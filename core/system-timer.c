#include <stdint.h>

#include "mem.h"
#include "system-timer.h"
#include "uart.h"

extern uint32_t mmio_base;

// TODO: Deduplicate this

// Memory-Mapped I/O output
static void mmio_write(uint32_t reg, uint32_t data) {
  write(mmio_base + reg, data);
}

// Memory-Mapped I/O input
static uint32_t mmio_read(uint32_t reg) {
  return read(mmio_base + reg);
}

uint32_t read_timer() {
  return mmio_read(STIMER_LO_COUNTER);
}

void write_timer_compare(char timer, uint32_t time) {
  uint32_t* addr = (uint32_t*)STIMER_COMPARE_0 + timer;
  mmio_write(addr, time);
}

void clear_timer_compare(char timer) {
  uint32_t* addr = (uint32_t*)STIMER_CONTROL;
  mmio_write(addr, 1 << timer);
}

void delay(uint32_t count) {
  uint32_t time = read_timer();
  uint32_t stop = time + count;

  while(time < stop) {
    time = read_timer();
  }
}
