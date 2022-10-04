#include <stddef.h>
#include <stdint.h>

#include "fb.h"
#include "uart.h"

extern size_t binary_entry;
extern uint32_t fb_base;

static void fb_write(uint32_t reg, uint32_t data) {
  *(volatile uint32_t*)(fb_base + reg) = data;
}

int get_pixel(int x, int y) {
  return y * FB_WIDTH + x;
}

void draw_rect(int top, int bottom, int left, int right) {
  for(int i = left; i < right; i++) // top
    fb_write(get_pixel(i, top) * 4, binary_entry);
  for(int i = left; i < right; i++) // bottom
    fb_write(get_pixel(i, bottom) * 4, binary_entry);
  for(int i = top; i < bottom; i++) // left
    fb_write(get_pixel(left, i) * 4, binary_entry);
  for(int i = top; i < bottom; i++) // right
    fb_write(get_pixel(right, i) * 4, binary_entry);
}

void fb_test() {
  uart_puts("Drawing rectangle...");
  draw_rect(500, 800, 1000, 1500);
  uart_puts(" Done!\r\n");
}

void draw_curve(int inner, int outer) {
  const int inner_len = inner * inner;
  const int outer_len = outer * outer;

  for(int y = 0; y < FB_HEIGHT; y++) {
    for(int x = 0; x < FB_WIDTH; x++) {
      int pixel = y * FB_WIDTH + x;

      int hyp2 = x * x + y * y;

      if(hyp2 > inner_len && hyp2 < outer_len) {
        fb_write(pixel * 4, binary_entry);
      }
    }
  }
}

void fb_test_2() {
  uart_puts("Drawing curve...");
  draw_curve(1000, 1080);
  uart_puts(" Done!\r\n");
}

void draw_image(uint32_t* image_data, int width, int height, int x_pos, int y_pos) {

  for(int y = 0; y < height; y++) {
    for(int x = 0; x < width; x++) {
      uint32_t color = *(image_data++);

      if(!(color >> 24))
        continue;

      fb_write(get_pixel(x + x_pos, y + y_pos) * 4, color);
    }
  }
}

extern const uint32_t _binary_logo_data_start;
extern const uint32_t _binary_logo_data_end;
extern const uint32_t _binary_logo_data_size;

#define LOGO_WIDTH  168
#define LOGO_HEIGHT 256

void draw_logo() {
  uart_puts("Drawing logo...");

  draw_image(
    (uint32_t*)&_binary_logo_data_start,
    LOGO_WIDTH, LOGO_HEIGHT, // width, height
    (FB_WIDTH / 2) - (LOGO_WIDTH / 2), // x pos
    (FB_HEIGHT / 2) - (LOGO_HEIGHT / 2) // y pos
  );

  uart_puts(" Done!\r\n");
}

void fb_clear() {
  uart_puts("Clearing frame buffer...");

  for(int i = 0; i < (FB_WIDTH * FB_HEIGHT); i++) {
    fb_write(i * 4, 0xff000000);
  }

  uart_puts(" Done!\r\n");
}