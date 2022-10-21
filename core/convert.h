#include <stddef.h>
#include <stdint.h>

void byte_to_hex(unsigned char b, void (*out)(unsigned char));
char char_as_hex(char input);
void word_to_hex(size_t word, void (*out)(unsigned char));

uint32_t swap_bytes(uint32_t value);

void read_digit(size_t* out, char digit);
void write_digits(size_t value, void (*out)(unsigned char));
