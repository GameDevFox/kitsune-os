#include <stddef.h>

char* hexTable = "0123456789abcdef";

char charAsHex(char input) {
	char binary;
	if(input >= '0' && input <= '9') // Number
		binary = (input - '0');
	else if(input >= 'a' && input <= 'f') // Letter
		binary = (input - 'a' + 10);
	else
		binary = 0xff;

	return binary;
}

void byteToHex(unsigned char b, void (*out)(unsigned char)) {
	unsigned char first4 = (b & 0xf0) >> 4;
	unsigned char last4 = b & 0xf;

	unsigned char firstChar = hexTable[first4];
	unsigned char secondChar = hexTable[last4];

	if(out) {
		out(firstChar);
	  out(secondChar);
	}
}

void wordToHex(size_t word, void (*out)(unsigned char)) {
	byteToHex(word >> 24 & 0xff, out);
	byteToHex(word >> 16 & 0xff, out);
	byteToHex(word >> 8  & 0xff, out);
	byteToHex(word >> 0  & 0xff, out);
}
