#include <byteswap.h>
#include <stdint.h>
#include <stdio.h>
#include <unistd.h>

#include "../hex.h"

int main(int argc, char** argv) {
  char* addressString = argv[1];

  int address = 0;
  int index = 0;
  char c;
  while(c = addressString[index]) {
    char i = charAsHex(c);
    int part = i << (28 - (index * 4));
    address |= part;
    // printf("%i %c %i %x %x\n", index, c, i, part, address);
    index++;
  }

  putc('W', stdout);
  putw(address, stdout);

  char in[4096];

  ssize_t count = fread(in, 1, 4096, stdin);
  putw(count, stdout);
  fwrite(in, 1, count, stdout);

  return 0;
}
