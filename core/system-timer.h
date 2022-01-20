#include <stdint.h>

uint32_t read_timer();
void write_timer_compare(char timer, uint32_t time);

enum {
  SYSTEM_TIMER_BASE = 0xfe003000,

    STIMER_CONTROL = (SYSTEM_TIMER_BASE + 0x00),
    STIMER_LO_COUNTER = (SYSTEM_TIMER_BASE + 0x04),
    STIMER_HI_COUNTER = (SYSTEM_TIMER_BASE + 0x08),

    STIMER_COMPARE_0 = (SYSTEM_TIMER_BASE + 0x0c),
    STIMER_COMPARE_1 = (SYSTEM_TIMER_BASE + 0x10),
    STIMER_COMPARE_2 = (SYSTEM_TIMER_BASE + 0x14),
    STIMER_COMPARE_3 = (SYSTEM_TIMER_BASE + 0x18),
};
