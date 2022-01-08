#include <stddef.h>
#include <stdint.h>

#include "fb.h"
#include "uart.h"

extern size_t binaryEntry;

static void fb_write(uint32_t reg, uint32_t data) {
	*(volatile uint32_t*)(FB_BASE + reg) = data;
}

int getPixel(int x, int y) {
	return y * FB_WIDTH + x;
}

void drawRect(int top, int bottom, int left, int right) {
  for(int i=left; i<right; i++) // top
		fb_write(getPixel(i, top) * 4, binaryEntry);
	for(int i=left; i<right; i++) // bottom
		fb_write(getPixel(i, bottom) * 4, binaryEntry);
	for(int i=top; i<bottom; i++) // left
		fb_write(getPixel(left, i) * 4, binaryEntry);
	for(int i=top; i<bottom; i++) // right
		fb_write(getPixel(right, i) * 4, binaryEntry);
}

void fbTest() {
	uart_puts("== FB TEST ==\r\n");
	drawRect(500, 800, 1000, 1500);
	uart_puts("== FB TEST DONE ==\r\n");
}

void drawCurve(int inner, int outer) {
  const int innerLen = inner * inner;
  const int outerLen = outer * outer;

	for(int y=0; y < FB_HEIGHT; y++) {
		for(int x=0; x < FB_WIDTH; x++) {
			int pixel = y * FB_WIDTH + x;

			int hyp2 = x * x + y * y;

			if(hyp2 > innerLen && hyp2 < outerLen) {
				fb_write(pixel * 4, binaryEntry);
			}
		}
	}
}

void fbTest2() {
	uart_puts("== FB TEST 2 ==\r\n");
  drawCurve(1000, 1080);
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

	// wordToHex(&_binary_logo_data_start, uart_putc); uart_puts("\r\n");
	// wordToHex(&_binary_logo_data_end, uart_putc); uart_puts("\r\n");
	// wordToHex(&_binary_logo_data_size, uart_putc); uart_puts("\r\n");

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
