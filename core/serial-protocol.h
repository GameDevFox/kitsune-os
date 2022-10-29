#define SP_START  0xff
#define SP_ESCAPE 0xfe
  #define SP_LITERAL_ESCAPE 0xfe
  #define SP_LITERAL_START  0x00
  #define SP_END            0x01
  #define SP_TARGET         0x02
  #define SP_LENGTH         0x03

void sp_start();
void sp_end();
void sp_target(unsigned char target);
void sp_putc(unsigned char c);
void sp_putw(uint32_t word);
void sp_puts(char* str);
void sp_write(char* str, uint32_t count);
void sp_log(char* str);

#define sp_frame(BODY) \
  sp_start(); \
  BODY \
  sp_end();
