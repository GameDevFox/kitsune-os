#ifndef RASPI_VERSION
#define RASPI_VERSION 4
#endif

static uint32_t MMIO_BASE;

// TODO: Move to seperate *.h and *.c file
size_t getMode();
size_t perfTest();

void uart_puts(const char* str);

uint32_t* addr;
uint32_t mem;

#define print_mem(addr) mem = *(uint32_t*)addr;\
	word_to_hex((uint32_t) addr, uart_putc);\
	uart_puts(": 0x");\
	word_to_hex(mem, uart_putc);
