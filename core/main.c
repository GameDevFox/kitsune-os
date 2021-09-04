#include <stddef.h>
#include <stdint.h>

#include "gic.h"
#include "hex.h"
#include "main.h"
#include "system-timer.h"
#include "tail.h"
#include "uart.h"

static char* hexTable = "0123456789abcdef";

void byte_to_hex(unsigned char b, void (*out)(unsigned char)) {
	unsigned char first4 = (b & 0xf0) >> 4;
	unsigned char last4 = b & 0xf;

	unsigned char firstChar = hexTable[first4];
	unsigned char secondChar = hexTable[last4];

	if(out) {
		out(firstChar);
	  out(secondChar);
	}
}

void word_to_hex(size_t word, void (*out)(unsigned char)) {
	byte_to_hex(word >> 24 & 0xff, out);
	byte_to_hex(word >> 16 & 0xff, out);
	byte_to_hex(word >> 8  & 0xff, out);
	byte_to_hex(word >> 0  & 0xff, out);
}

void printCycleCounterList() {
	uart_puts("CYCLE COUNTER LIST:\r\n");
	size_t prev = 0;
	for(int i=0; i<8; i++) {
		size_t* address = (void*)0x40 + (i * 4);
		printAddress(address, uart_putc);

		size_t current = *address;
		size_t delta = current - prev;
		prev = current;

		if(i != 0) {
			uart_puts(" :: ");
			word_to_hex(delta, uart_putc);
		}

		uart_puts("\r\n");
	}
}

void printParams(uint32_t r0, uint32_t r1, uint32_t atags) {
	uart_puts("r0:    0x");
	word_to_hex(r0, uart_putc);
	uart_puts("\r\n");

	uart_puts("r1:    0x");
	word_to_hex(r1, uart_putc);
	uart_puts("\r\n");

	uart_puts("atags: 0x");
	word_to_hex(atags, uart_putc);
	uart_puts("\r\n");

	uart_puts("\r\n");
}

#define MRC_TEMPLATE 0xee100010

size_t generateMRC(
	char coproc, char opc1, char Rt,
	char CRn, char CRm, char opc2
) {
	size_t result = MRC_TEMPLATE;

	result |= coproc << 8;
	result |= opc1   << 21;
	result |= Rt     << 12;
	result |= CRn    << 16;
	result |= CRm    << 0;
	result |= opc2   << 5;

	return result;
}

char getb() {
	char input = uart_getc();
	return charAsHex(input);
}

size_t binaryEntry = 0xffff5500;

void binaryEntryMode() {
	uart_puts("== ENTRY MODE ==\r\n");

	char input = 0;

	char running = 1;
	while(running) {
		word_to_hex(binaryEntry, uart_putc);
		uart_puts("\r\n");

		input = uart_getc();

		if(input == 0x0d) // ENTER
			break;

		if(input == 0x7f) // DEL
			binaryEntry = (binaryEntry >> 4);
		else {
			char value = charAsHex(input);

			if(value != 0xff)
				binaryEntry = (binaryEntry << 4) | value;
			else {
				uart_puts("Oops...\r\n");
				continue;
			}
		}
	}

	uart_puts("== INPUT MODE ==\r\n");
}

void walkMemory(size_t* start, void (*out)(unsigned char)) {
	uart_puts("== MEMORY WALK ==\r\n");

	size_t* current = start;

	char increment = 1;
	while(1) {
		while(increment) {
			printAddress(current, out);
			out('\r');
			out('\n');

			current++;
			increment--;
		}

		char input = uart_getc();
		if(input == '0' || input == 0x1b) // '0' or Escape
			break;

		char hex = charAsHex(input);
		if(hex == 0xff) {
			increment = 1;
		} else {
			increment = hex;
			uart_puts("--------\r\n");
		}
	}

	uart_puts("== WALK FINISHED ==\r\n");
}

#define FB_BASE 0x3e3cf000

static void fb_write(uint32_t reg, uint32_t data) {
	*(volatile uint32_t*)(FB_BASE + reg) = data;
}

const int width = 1920;
const int height = 1080;
const int total = width * height;

const int a = 1000;
const int b = 1080;

const int lenA = a * a;
const int lenB = b * b;

int getPixel(int x, int y) {
	return y * width + x;
}

void fbTest() {
	uart_puts("== FB TEST ==\r\n");

	int top = 500;
	int bottom = 800;
	int left = 1000;
	int right = 1500;

	for(int i=left; i<right; i++) // top
		fb_write(getPixel(i, top) * 4, binaryEntry);
	for(int i=left; i<right; i++) // bottom
		fb_write(getPixel(i, bottom) * 4, binaryEntry);
	for(int i=top; i<bottom; i++) // left
		fb_write(getPixel(left, i) * 4, binaryEntry);
	for(int i=top; i<bottom; i++) // right
		fb_write(getPixel(right, i) * 4, binaryEntry);

	uart_puts("== FB TEST DONE ==\r\n");
}

void fbTest2() {
	uart_puts("== FB TEST 2 ==\r\n");

	for(int y=0; y < height; y++) {
		for(int x=0; x < width; x++) {
			int pixel = y * width + x;

			int hyp2 = x * x + y * y;

			if(hyp2 > lenA && hyp2 < lenB) {
				fb_write(pixel * 4, binaryEntry);
			}
		}
	}

	uart_puts("== FB TEST 2 DONE ==\r\n");
}

extern const uint32_t _binary_logo_data_start;
extern const uint32_t _binary_logo_data_end;
extern const uint32_t _binary_logo_data_size;

void drawImage(uint32_t* imageData, int width, int height, int xPos, int yPos) {

	for(int y=0; y<height; y++) {
		for(int x=0; x<width; x++) {
			uint32_t color = *(imageData++);

			if(!(color >> 24))
				continue;

			fb_write(getPixel(x + xPos, y + yPos) * 4, color);
		}
	}
}

void drawLogo() {
	uart_puts("Drawing logo to frame buffer...");

	// word_to_hex(&_binary_logo_data_start, uart_putc); uart_puts("\r\n");
	// word_to_hex(&_binary_logo_data_end, uart_putc); uart_puts("\r\n");
	// word_to_hex(&_binary_logo_data_size, uart_putc); uart_puts("\r\n");

	drawImage(
		(uint32_t*) &_binary_logo_data_start,
		168, 256, // width, height
		876, 412 // x, y
	);

	uart_puts(" Done!\r\n");
}

void fbClear() {
	uart_puts("== FB CLEAR ==\r\n");

	for(int i=0; i<(1080 * 1920); i++) {
		fb_write(i * 4, 0xff000000);
	}

	uart_puts("== FB CLEAR DONE ==\r\n");
}

size_t uart_getw() {
	size_t word = 0;
	word |= (uart_getc() << 0);
	word |= (uart_getc() << 8);
	word |= (uart_getc() << 16);
	word |= (uart_getc() << 24);

	return word;
}

void readMemory() {
	size_t size = uart_getw();

	for(size_t i=binaryEntry; i<binaryEntry+size; i++) {
		char value = *(char*)i;
		uart_putc(value);
	}
}

void writeMemory() {
	size_t start = uart_getw();
	size_t size = uart_getw();

	uart_puts("Writing ");
	word_to_hex(size, uart_putc);
	uart_puts(" bytes to ");
	word_to_hex(start, uart_putc);
	uart_puts(" ...");

	size_t end = start+size;

	for(size_t i=start; i<end; i++) {
		char value = uart_getc();
		*(char*)i = value;
	}

	uart_puts(" Done!\r\n");
}

#define SWI(value) "swi #" #value

char isPollingEnabled = 1;

void input_loop() {
	char input = 0;

	while (1) {
		if(isPollingEnabled)
			input = uart_getc();

		switch(input) {
			case '=':
				binaryEntryMode();
				break;
			case '~':
				// Generate MRC instuction
				uart_puts("coproc\r\n");
				char coproc = getb();
				uart_puts("opc1\r\n");
				char opc1 = getb();
				// uart_puts("Rt\r\n");
				// char Rt = getb();
				uart_puts("CRn\r\n");
				char CRn = getb();
				uart_puts("CRm\r\n");
				char CRm = getb();
				uart_puts("opc2\r\n");
				char opc2 = getb();

				char Rt = 0;
				binaryEntry = generateMRC(
					coproc, opc1, Rt, CRn, CRm, opc2
				);
				word_to_hex(binaryEntry, uart_putc);
				uart_puts("\r\n");
				break;
			case '!':
				// Write instruction to our function
				*(volatile size_t*)(&liveInstruction) = binaryEntry;

				// Call function and print result
				size_t result = liveFn();
				word_to_hex(result, uart_putc);
				uart_puts("\r\n");
				break;

			case 0x0d: // ENTER
				uart_puts("\r\n");
				break;
			case 0x7f: // DEL
				// TODO: Fix this hacky way, there must be a better way
				uart_putc('\b');
				uart_putc(' ');
				uart_putc('\b');
				break;

			case '1':
				uart_puts("Hello, Kitsune!\r\n");
				break;
			case '2':
				asm(SWI(1234));
				break;
			case '3':
				size_t mode = getCPSR();
				word_to_hex(mode, uart_putc);
				uart_puts("\r\n");
				break;
			case '4':
				perfTest();
				printCycleCounterList();
				uart_puts("Done\r\n" "\r\n");
				break;
			case '5':
				asm(".word 0xffffffff");
				break;
			case '6':
				fbTest();
				break;
			case '7':
				fbTest2();
				break;
			case '9':
				drawLogo();
				break;
			case '0':
				fbClear();
				break;
			case '@':
				uart_puts("Disabled...\r\n");
				input = 0;
				isPollingEnabled = 0;
				break;

			case 'q':
				size_t intMode = toggle_irqs();
				word_to_hex(intMode, uart_putc);
				uart_puts("\r\n");
				break;
			case 'r':
				walkMemory((size_t*)binaryEntry, uart_putc);
				break;
			case 'w':
				uart_puts("Enter hex to write to ");
				word_to_hex(binaryEntry, uart_putc);
				uart_puts("\r\n");

				uint32_t value = 0;
				value |= getb() << 0x1c;
				value |= getb() << 0x18;
				value |= getb() << 0x14;
				value |= getb() << 0x10;
				value |= getb() << 0x0c;
				value |= getb() << 0x08;
				value |= getb() << 0x04;
				value |= getb() << 0x00;

				*(uint32_t*)binaryEntry = value;

				uart_puts("Wrote ");
				word_to_hex(value, uart_putc);
				uart_puts("\r\n");
				break;

			case 'R':
				readMemory();
				break;
			case 'W':
				writeMemory();
				break;

			case 't':
				uint32_t time = readTimer();
				word_to_hex(time, uart_putc);
				uart_puts("\r\n");
				break;
			case 'T':
				tailFn();
				break;

			case 'z':
				uart_puts("enableIRQ and routeIRQtoCPUS\r\n");
				enableIRQ(VC_IRQ_TIMER_1);
				routeIRQtoCPUS(VC_IRQ_TIMER_1, 0x01);
				uart_puts("Done\r\n");
				break;
			case 'x':
				uart_puts("writeTimerCompare\r\n");
				writeTimerCompare(1, binaryEntry);
				uart_puts("Wrote Timer Compare\r\n");
				break;
			case 'c':
				uart_puts("Set time 3 seconds from now\r\n");
				uint32_t t = readTimer();
				writeTimerCompare(1, t + (3 * 1000000));
				uart_puts("Done\r\n");
				break;
			// case 'v':
			// 	break;

			default:
				uart_putc(input);
				break;
		}
	}
}

void memoryRangeOut(size_t* start, size_t count, void (*out)(unsigned char)) {
	size_t* end = start + count;

	for(size_t* i = start; i < end; i++) {
		printAddress(i, out);
		out('\r');
		out('\n');
	}

	out('\r');
	out('\n');
}

struct deviceTreeHeader {
	uint32_t magic;
	uint32_t totalSize;

	uint32_t structureOffset;
	uint32_t stringsOffset;
	uint32_t memoryReservationOffset;

	uint32_t version;
	uint32_t lastCompatibleVersion;

	uint32_t bootCpuPhysicalId;

	uint32_t stringsSize;
	uint32_t structureSize;
};

#if defined(__cplusplus)
extern "C" /* Use C linkage for kernel_main. */
#endif

#ifdef AARCH64
// arguments for AArch64
void kernel_main(uint64_t dtb_ptr32, uint64_t x1, uint64_t x2, uint64_t x3)
#else
// arguments for AArch32
void kernel_main(uint32_t r0, uint32_t r1, uint32_t atags)
#endif
{
	// Initialize UART for Raspberry Pi
	uart_init();

	uart_puts("Hello, Kitsune!\r\n");
	uart_puts("\r\n");

	printParams(r0, r1, atags);

	// uart_puts("DEVICE TREE:\r\n");
	// memoryRangeOut((size_t*)atags, 32, uart_putc);

	drawLogo();

	uart_puts("READY\r\n");

	input_loop();
}
