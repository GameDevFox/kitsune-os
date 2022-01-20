#include <stdint.h>

#include "mem.h"
#include "system-timer.h"
#include "uart.h"

uint32_t read_timer() {
  return read(STIMER_LO_COUNTER);
}

void write_timer_compare(char timer, uint32_t time) {
  uint32_t* addr = (uint32_t*)STIMER_COMPARE_0 + timer;
  write(addr, time);
}

void clear_timer_compare(char timer) {
  uint32_t* addr = (uint32_t*)STIMER_CONTROL;
  write(addr, 1 << timer);
}
