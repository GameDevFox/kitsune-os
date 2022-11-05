#define FB_WIDTH 1920
#define FB_HEIGHT 1080

void fb_clear();

void fb_test();
void fb_test_2();
void draw_mascot();
void draw_no_glasses();
void draw_logo();

void draw_char(char c, uint32_t x, uint32_t y);
void draw_string(char* str, uint32_t x, uint32_t y);
void draw_string_animated(char* str, uint32_t x, uint32_t y, uint32_t delay_count);
