#include <stdint.h>

#include "mem.h"

#include "system-timer.h"

uint32_t read_timer() {
  return mmio_read(STIMER_LO_COUNTER);
}

void write_timer_compare(char timer, uint32_t time) {
  uint32_t addr = STIMER_COMPARE_0 + (timer * 4);
  mmio_write(addr, time);
}

void clear_timer_compare(char timer) {
  uint32_t addr = STIMER_CONTROL;
  mmio_write(addr, 1 << timer);
}

void delay(uint32_t count) {
  uint32_t time = read_timer();
  uint32_t stop = time + count;

  while(time < stop) {
    time = read_timer();
  }
}
