#include "convert.h"

char* hex_table = "0123456789abcdef";

char char_as_hex(char input) {
  char binary;
  if(input >= '0' && input <= '9') // Number
    binary = (input - '0');
  else if(input >= 'a' && input <= 'f') // Letter
    binary = (input - 'a' + 10);
  else
    binary = 0xff;

  return binary;
}

void byte_to_hex(unsigned char b, void (*out)(unsigned char)) {
  unsigned char first4 = (b & 0xf0) >> 4;
  unsigned char last4 = b & 0xf;

  unsigned char first_char = hex_table[first4];
  unsigned char second_char = hex_table[last4];

  if(out) {
    out(first_char);
    out(second_char);
  }
}

void word_to_hex(size_t word, void (*out)(unsigned char)) {
  byte_to_hex(word >> 24 & 0xff, out);
  byte_to_hex(word >> 16 & 0xff, out);
  byte_to_hex(word >> 8 & 0xff, out);
  byte_to_hex(word >> 0 & 0xff, out);
}

uint32_t swap_bytes(uint32_t value) {
  uint32_t result = 0;

  result = result | (value >> 24 & 0xff);
  result = result | (value >> 8 & 0xff00);
  result = result | (value << 8 & 0xff0000);
  result = result | (value << 24 & 0xff000000);

  return result;
}

void read_digit(size_t* out, char digit) {
  *(out) *= 10;
  char value = digit - 0x30;
  *(out) += value;
}

void do_write_digits(size_t value, void (*out)(unsigned char)) {
  if(!value)
    return;

  size_t digit = value % 10;
  char c = hex_table[digit];
  do_write_digits(value / 10, out);
  out(c);
}

void write_digits(size_t value, void (*out)(unsigned char)) {
  if(!value) {
    out('0');
    return;
  }

  do_write_digits(value, out);
}
