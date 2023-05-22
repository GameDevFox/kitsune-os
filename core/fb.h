#define FB_WIDTH 1920
#define FB_HEIGHT 1080

void fb_clear();
void clear_pixels();

void fb_test();
void fb_test_2();
void draw_aki();
void draw_mascot();
void draw_glasses();
void draw_logo();

void draw_char(char c, const uint32_t* font, uint32_t x, uint32_t y);
void draw_string(char* str, const uint32_t* font, uint32_t x, uint32_t y);
void draw_string_animated(char* str, const uint32_t* font, uint32_t x, uint32_t y, uint32_t delay_count);
