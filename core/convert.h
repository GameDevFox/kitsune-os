void byteToHex(unsigned char b, void (*out)(unsigned char));
char charAsHex(char input);
void wordToHex(size_t word, void (*out)(unsigned char));

void read_digit(size_t *out, char digit);
void write_digits(size_t value, void (*out)(unsigned char));
