#include <stddef.h>
#include <stdint.h>

#include "../arch/arm/asm.h"

#include "convert.h"
#include "fb.h"
#include "output.h"
#include "system-timer.h"
#include "uart.h"

extern size_t binary_entry;
extern void* fb_base;

void fb_clear() {
  uart_puts("Clearing frame buffer...");

  clear_bytes(fb_base, FB_WIDTH * FB_HEIGHT * 4);

  uart_puts(" Done!" EOL);
}

static void fb_write(uint32_t reg, uint32_t data) {
  *(volatile uint32_t*)(fb_base + reg) = data;
}

int get_pixel_offset(int x, int y) {
  return y * FB_WIDTH + x;
}

void draw_rect(int top, int bottom, int left, int right) {
  for(int i = left; i < right; i++) // top
    fb_write(get_pixel_offset(i, top) * 4, binary_entry);
  for(int i = left; i < right; i++) // bottom
    fb_write(get_pixel_offset(i, bottom) * 4, binary_entry);
  for(int i = top; i < bottom; i++) // left
    fb_write(get_pixel_offset(left, i) * 4, binary_entry);
  for(int i = top; i < bottom; i++) // right
    fb_write(get_pixel_offset(right, i) * 4, binary_entry);
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

      int hyp2 = (x - FB_WIDTH) * (x - FB_WIDTH) + y * y;

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

void clear_pixels(
  uint32_t x, uint32_t y,
  uint32_t width, uint32_t height
) {
  while(y < height) {
    uint32_t pixelOffset = get_pixel_offset(x, y) * 4;
    clear_bytes(fb_base + pixelOffset, width * 4);
    y++;
  }
}

// TODO: Make a wrapper function that clips the bounds of the
// render to the screen boundaries (0 < x < FB_WIDTH, same for y)
// TODO: Look over this again and see if we can simplify the math
void draw_image_base(
  const uint32_t* image_data,
  uint32_t src_x, uint32_t src_y,
  uint32_t src_width, uint32_t src_height,
  uint32_t dest_x, uint32_t dest_y,
  uint32_t dest_width, uint32_t dest_height
) {
  if(dest_width == 0)
    dest_width = src_width;
  if(dest_height == 0)
    dest_height = src_height;

  image_data += src_y * src_width;

  for(
    uint32_t y = src_y;
    (
      y < src_height &&
      y < src_y + dest_height &&
      y - src_y + dest_y < FB_HEIGHT
    );
    y++
  ) {
    image_data += src_x;

    for(uint32_t x = src_x; x < src_width && x < src_x + dest_width && x < FB_WIDTH; x++) {
      const uint32_t* image_data_start = image_data;

      uint32_t color = *image_data++;
      if(color >> 24 != 0xff)
        continue;

      uint32_t length = 1;
      while((x - src_x) + length < dest_width) {
        color = *image_data++;
        if(color >> 24 != 0xff)
          break;

        length++;
      }

      uint32_t pixelOffset = get_pixel_offset(dest_x - src_x + x, dest_y - src_y + y) * 4;
      copy_bytes(image_data_start, fb_base + pixelOffset, length * 4);

      x += length;
    }

    image_data += src_width - (src_x + dest_width);
  }
}

void draw_image(uint32_t* image_data, uint32_t dest_x, uint32_t dest_y) {
  uint32_t src_width = *image_data++;
  uint32_t src_height = *image_data++;

  draw_image_base(
    image_data,
    0, 0, src_width, src_height,
    dest_x, dest_y, 0, 0
  );
}

// Logo

#define LOGO_WIDTH  168
#define LOGO_HEIGHT 256

extern const uint32_t _binary_logo_data_start;

void draw_logo() {
  uart_puts("Drawing logo...");

  draw_image(
    (uint32_t*) &_binary_logo_data_start,
    (FB_WIDTH / 2) - (LOGO_WIDTH / 2), // x pos
    (FB_HEIGHT / 2) - (LOGO_HEIGHT / 2) // y pos
  );

  uart_puts(" Done!\r\n");
}

#define DRAW_IMAGE(NAME, X, Y) \
extern const uint32_t _binary_ ## NAME ## _data_start; \
\
void draw_ ## NAME() { \
  draw_image( \
    (uint32_t*) &_binary_ ## NAME ## _data_start, \
    X, Y \
  ); \
} \

// Aki

#define AKI_X 1100
#define AKI_Y 100

DRAW_IMAGE(aki_base, AKI_X, AKI_Y)

#define AKI_GLASSES_X 308
#define AKI_GLASSES_Y 172

DRAW_IMAGE(aki_glasses, AKI_X + AKI_GLASSES_X, AKI_Y + AKI_GLASSES_Y)
DRAW_IMAGE(aki_no_glasses, AKI_X + AKI_GLASSES_X, AKI_Y + AKI_GLASSES_Y)

#define AKI_MOUTH_X 368
#define AKI_MOUTH_Y 220

DRAW_IMAGE(aki_mouth0, AKI_X + AKI_MOUTH_X, AKI_Y + AKI_MOUTH_Y)
DRAW_IMAGE(aki_mouth1, AKI_X + AKI_MOUTH_X, AKI_Y + AKI_MOUTH_Y)
DRAW_IMAGE(aki_mouth2, AKI_X + AKI_MOUTH_X, AKI_Y + AKI_MOUTH_Y)

// Mascot

#define MASCOT_X 510
#define MASCOT_Y 280

#define MASCOT_GLASSES_X 169
#define MASCOT_GLASSES_Y 200

extern const uint32_t _binary_mascot_data_start;

void draw_mascot() {
  uart_puts("Drawing mascot...");

  draw_image(
    (uint32_t*) &_binary_mascot_data_start,
    MASCOT_X, MASCOT_Y // x, y
  );

  uart_puts(" Done!\r\n");
}

extern const uint32_t _binary_glasses_data_start;

void draw_glasses() {
  uart_puts("Drawing glasses...");

  draw_image(
    (uint32_t*) &_binary_glasses_data_start,
    MASCOT_X + MASCOT_GLASSES_X, MASCOT_Y + MASCOT_GLASSES_Y // x, y
  );

  uart_puts(" Done!\r\n");
}

void draw_char(char c, const uint32_t* font, uint32_t x, uint32_t y) {
  uint32_t src_width = *font++;
  uint32_t src_height = *font++;

  uint32_t char_width = src_width / 16;
  uint32_t char_height = src_height / 8;

  uint32_t src_x = (c % 16) * char_width;
  uint32_t src_y = (c / 16) * char_height;

  uint32_t dest_width = src_width / 16;
  uint32_t dest_height = src_height / 8;

  draw_image_base(
    font,
    src_x, src_y, src_width, src_height,
    x, y, dest_width, dest_height
  );
}

void draw_string(char* str, const uint32_t* font, uint32_t x, uint32_t y) {
  uint32_t src_width = font[0];
  uint32_t src_height = font[1];

  uint32_t char_height = src_height / 8;

  uint32_t my_x = x;
  for(int i=0; str[i] != 0; i++) {
    char c = str[i];
    if(c == '\n') {
      my_x = x;
      y += char_height;
      continue;
    }

    draw_char(c, font, my_x, y);

    my_x += src_width / 16;
  }
}

void (*draw_mouths[])(void) = {
  draw_aki_mouth1,
  draw_aki_mouth2,
  draw_aki_mouth1,
  draw_aki_mouth0,
};

#define MOUTHS_LENGTH (sizeof(draw_mouths) / sizeof(draw_mouths[0]))

void draw_string_animated(
  char* str, const uint32_t* font,
  uint32_t x, uint32_t y,
  uint32_t delay_count, uint32_t mouth_delay
) {
  if(delay_count == 0)
    delay_count = 100000;

  uint32_t src_width = font[0];
  uint32_t src_height = font[1];

  uint32_t char_height = src_height / 8;

  uint32_t delay_counter = 0;

  uint32_t my_x = x;
  for(int i=0; str[i] != 0; i++) {
    char c = str[i];
    if(c == ' ')
      goto end;

    if(c == '\n') {
      my_x = x;
      y += char_height;
      continue;
    }

    if(mouth_delay != 0) {
      uint32_t mouth_index = delay_counter / mouth_delay;
      void (*draw_mouth)(void) = draw_mouths[mouth_index % MOUTHS_LENGTH];
      draw_mouth();
      delay_counter += delay_count;
    }

    draw_char(c, font, my_x, y);
    delay(delay_count);

    end:;
    my_x += src_width / 16;
  }

  if(mouth_delay != 0)
    draw_aki_mouth0();
}
