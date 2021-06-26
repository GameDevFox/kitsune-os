#ifndef RASPI_VERSION
#define RASPI_VERSION 4
#endif

// TODO: Move to seperate *.h and *.c file
size_t getMode();
size_t perfTest();

size_t liveFn();
size_t liveInstruction();

void uart_puts(const char* str);
void initialRegisterValues();

uint32_t mem;

#define printAddress(addr, out) mem = *(uint32_t*)addr;\
	word_to_hex((uint32_t) addr, out);\
	out(':');\
	out(' ');\
	out('0');\
	out('x');\
	word_to_hex(mem, out);
