#include <stdint.h>

#define write(addr, data) (*(volatile uint32_t*)(addr) = (data))
#define read(addr) (*(volatile uint32_t*)(addr))

void mmio_write(uint32_t reg, uint32_t data);
uint32_t mmio_read(uint32_t reg);
