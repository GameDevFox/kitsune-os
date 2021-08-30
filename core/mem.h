#include <stdint.h>

#define write(addr, data) (*(volatile uint32_t*)(addr) = (data))
#define read(addr) (*(volatile uint32_t*)(addr))
