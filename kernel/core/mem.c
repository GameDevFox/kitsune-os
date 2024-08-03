#include <stdint.h>

#include "./section.h"

#include "./mem.h"

extern uint32_t mmio_base;

__eden void mmio_write(uint32_t reg, uint32_t data) {
  write(mmio_base + reg, data);
}

__eden uint32_t mmio_read(uint32_t reg) {
  return read(mmio_base + reg);
}
