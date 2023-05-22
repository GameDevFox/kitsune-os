#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "../arch/arm/asm.h"

#include "arm.h"
#include "convert.h"
#include "device-tree.h"
#include "fb.h"
#include "gic.h"
#include "input.h"
#include "output.h"
#include "serial-protocol.h"
#include "system-timer.h"
#include "uart.h"
#include "vt.h"

extern size_t binary_entry;

void print_address(size_t* addr, void (*out)(unsigned char)) {
  uint32_t mem = *(uint32_t*) addr;

  word_to_hex((uint32_t) addr, out); uart_puts(": 0x");
  word_to_hex(mem, out);
}

void print_cycle_counter_list() {
  uart_puts("CYCLE COUNTER LIST:" EOL);

  size_t prev = 0;
  for(int i = 0; i < 8; i++) {
    size_t* address = (void*)0x40 + (i * 4);
    print_address(address, uart_putc);

    size_t current = *address;
    size_t delta = current - prev;
    prev = current;

    if(i != 0) {
      uart_puts(" :: ");
      word_to_hex(delta, uart_putc);
    }

    uart_puts(EOL);
  }
}

void read_memory() {
  char target = uart_getc();
  uint32_t start = uart_getw();
  uint32_t length = uart_getw();

  sp_frame(
    sp_target(target);
    sp_write((char*) start, length);
  );
}

void write_memory() {
  size_t start = uart_getw();
  size_t size = uart_getw();

  uart_puts("Writing ");
  word_to_hex(size, uart_putc);
  uart_puts(" bytes to ");
  word_to_hex(start, uart_putc);
  uart_puts(" ...");

  size_t end = start + size;

  for(size_t i = start; i < end; i++) {
    char value = uart_getc();
    *(char*)i = value;
  }

  uart_puts(" Done!" EOL);
}

int run_loop = 1;
bool is_polling_enabled = true;

void backspace() {
  // TODO: Fix this hacky way, there must be a better way
  uart_putc('\b');
  uart_putc(' ');
  uart_putc('\b');
}

void print_timer() {
  uint32_t t = read_timer();
  uint32_t p = get_pmccntr();
  word_to_hex(t, uart_putc);
  uart_puts(EOL);
  word_to_hex(p, uart_putc);
  uart_puts(EOL EOL);
}

uint32_t call_instruction(uint32_t instruction, uint32_t arg) {
  // Write instruction to our function
  *(volatile size_t*)(&live_instruction) = instruction;

  // Call function and print result
  return live_fn(arg);
}

void generate_mrc(bool is_mrc) {
  char target = uart_getc();

  char coproc = getb();
  char opc1 = getb();
  char cr_n = getb();
  char cr_m = getb();
  char opc2 = getb();

  uint32_t value = is_mrc ? 0 : uart_getw();

  char rt = 0; // Register
  uint32_t flags = is_mrc ? ASM_MRC_FLAG : 0;
  uint32_t instruction = asm_mrc_mcr(coproc, opc1, rt, cr_n, cr_m, opc2, flags);
  uint32_t result = call_instruction(instruction, value);

  sp_frame(
    sp_target(target);
    sp_putw(result);
  );
}

void write_word() {
  uart_puts("Enter hex to write to ");
  word_to_hex(binary_entry, uart_putc);
  uart_puts(EOL);

  uint32_t value = 0;
  value |= getb() << 0x1c;
  value |= getb() << 0x18;
  value |= getb() << 0x14;
  value |= getb() << 0x10;
  value |= getb() << 0x0c;
  value |= getb() << 0x08;
  value |= getb() << 0x04;
  value |= getb() << 0x00;

  *(uint32_t*)binary_entry = value;

  uart_puts("Wrote ");
  word_to_hex(value, uart_putc);
  uart_puts(EOL);
}

void init_irq() {
  uart_puts("enableIRQ and routeIRQtoCPUS" EOL);
  enable_irq(VC_IRQ_TIMER_1);
  route_irq_to_cpus(VC_IRQ_TIMER_1, 0x01);
  uart_puts("Done" EOL);
}

#define ONE_SECOND 1000000

void in_3_seconds() {
  uart_puts("Set time 3 seconds from now" EOL);
  uint32_t t = read_timer();
  write_timer_compare(1, t + (3 * ONE_SECOND));
  uart_puts("Done" EOL);
}

void print_cpsr() {
  char target = uart_getc();
  char writeFlag = uart_getc();

  if(writeFlag) {
    uint32_t value = uart_getw();
    set_cpsr(value);

    sp_frame(
      sp_target(target);
    );
  } else {
    uint32_t mode = get_cpsr();

    sp_frame(
      sp_target(target);
      sp_putw(mode);
    );
  }
}

void performance_test() {
  perf_test();
  print_cycle_counter_list();
  uart_puts("Done" EOL EOL);
}

void do_halt() {
  run_loop = 0;
  uart_puts("HALT" EOL);
}

void disable_polling() {
  uart_puts("Disabled..." EOL);
  is_polling_enabled = 0;
}

void do_toggle_irq() {
  size_t int_mode = toggle_irqs();
  word_to_hex(int_mode, uart_putc);
  uart_puts(EOL);
}

void do_set_mode() {
  set_mode(binary_entry);
  binary_entry = get_cpsr();
  word_to_hex(binary_entry, uart_putc);
  uart_puts(EOL);
}

void do_enable_cache() {
  enable_cache();
  uart_puts("Cache enabled" EOL);
}

void print_cursor_pos() {
  struct VT_Size pos = vt_get_pos();

  uart_puts("Row: ");
  word_to_hex(pos.row, uart_putc);
  uart_puts(" Column: ");
  word_to_hex(pos.column, uart_putc);
  uart_puts(EOL);
}

void print_vt_size() {
  struct VT_Size size = vt_get_size();

  uart_puts("Rows: ");
  word_to_hex(size.row, uart_putc);
  uart_puts(" Columns: ");
  word_to_hex(size.column, uart_putc);
  uart_puts(EOL);
}

void print_performance_counter() {
  uart_puts("Performance: ");
  size_t count = get_performance_counter();
  word_to_hex(count, uart_putc);
  uart_puts(EOL);
}

extern const uint32_t _binary_character_sheet_base_data_start[];
extern const uint32_t _binary_character_sheet_micro_data_start[];
extern const uint32_t _binary_character_sheet_nano_data_start[];

#define base_font _binary_character_sheet_base_data_start
#define micro_font _binary_character_sheet_micro_data_start
#define nano_font _binary_character_sheet_nano_data_start

void draw_kitsune_text() {
  uint32_t str_width = 32 * 7; // 32 pixels per char
  uint32_t str_x =  (FB_WIDTH / 2)  - (str_width / 2);
  draw_string_animated("KITSUNE", base_font, str_x, 690, 0);
}

void draw_mascot_text() {
  char* mascot_text_str =
    "Welcome to Kitsune!\n"
    "Let's learn and have\n"
    "fun together.";
  draw_string_animated(mascot_text_str, micro_font, 1000, 180, 50000);
}

char uptime_str[16];
uint32_t uptime_str_index = 0;

void set_uptime_str(unsigned char c) {
  uptime_str[uptime_str_index++] = c;
}

void draw_clock(uint32_t seconds, uint32_t minutes, uint32_t hours) {
  uint32_t char_width = base_font[0] / 16;
  uint32_t char_height = base_font[1] / 8;

  uint32_t x = FB_WIDTH - (10 * char_width) - 4; // 8 chars, 4 px padding

  // Clear old clock
  clear_pixels(x, 0, FB_WIDTH - x, char_height);

  // draw hours
  uptime_str_index = 0;
  word_to_dec(hours, set_uptime_str);
  uptime_str[uptime_str_index++] = ':';
  uptime_str[uptime_str_index++] = 0;

  draw_string(uptime_str+6, base_font, x + (char_width * 0), 0);

  // draw minutes
  uptime_str_index = 0;
  word_to_dec(minutes, set_uptime_str);
  uptime_str[uptime_str_index++] = ':';
  uptime_str[uptime_str_index++] = 0;

  draw_string(uptime_str+8, base_font, x + (char_width * 5), 0);

  // draw seconds
  uptime_str_index = 0;
  word_to_dec(seconds, set_uptime_str);
  uptime_str[uptime_str_index++] = 0;

  draw_string(uptime_str+8, base_font, x + (char_width * 8), 0);
}

void draw_uptime_clock() {
  static uint32_t last_tick = 0;
  static uint32_t second_counter = 0;
  static uint32_t accumulator = 0;

  uint32_t latest_tick = read_timer();

  uint32_t diff;
  if(latest_tick == last_tick) {
    return;
  } else if(latest_tick > last_tick)
    diff = (latest_tick - last_tick); // No Wrap
  else
    diff = (0xffffffff - last_tick) + latest_tick; // Timer wrapped around
  last_tick = latest_tick;

  accumulator += diff;
  if(accumulator < ONE_SECOND)
    return;

  // Update second_counter
  second_counter += (accumulator / ONE_SECOND);
  accumulator %= ONE_SECOND;

  uint32_t seconds = second_counter;
  uint32_t minutes = seconds / 60 % 60;
  uint32_t hours = seconds / 3600;
  seconds %= 60;

  draw_clock(seconds, minutes, hours);
}

void set_binary_entry() {
  binary_entry = uart_getw();
}

void (*char_handler)(char) = NULL;

void command_handler(char);

void raw_output(char c) {
  if(c == '\e') // ESCAPE
    char_handler = command_handler;
  else
    uart_putc(c);
}

size_t last_count = 0;
size_t counter = 0;

#define ENTER 0x0d
#define ESCAPE 0x1b
#define BACKSPACE 0x7f

void main_handler(char);
void set_input_handler(void (*handler)(char));

void (*input_handler)(char) = command_handler;

void raw_handler(char c) {
  byte_to_hex(c, uart_putc);
  uart_putc(' ');
}

void* device_tree = 0;

void command_handler(char input) {
  switch(input) {
    case '`': generate_mrc(1); break;
    case '~': generate_mrc(0); break;

    case '1': uart_puts("Hello, Kitsune!" EOL); break;
    case '2': asm(SWI(1234)); break;
    case '3': print_cpsr(); break;
    case '4': performance_test(); break;
    case '5': asm(".word 0xffffffff"); break;
    case '6': fb_test(); break;
    case '7': fb_test_2(); break;
    case '8': draw_mascot(); break;
    case '9': draw_logo(); break;
    case '0': fb_clear(); break;

    case '!': call_instruction(binary_entry, 0); break;
    case '@': disable_polling(); break;
    case '#': draw_aki(); break;
    case '$': draw_aki_glasses(); break;
    case '%': draw_aki_no_glasses(); break;
    case '=': binary_entry_mode(); break;

    case 'q': do_toggle_irq(); break;
    case 'w': write_word(); break;
    case 'e': set_binary_entry(); break;
    case 'r': draw_kitsune_text(); break;
    case 't': print_timer(); break;
    case 'y': draw_mascot_text(); break;

    case 'a': uart_puts(VT_SAVE VT_HOME VT_RED "Red Text" EOL VT_DEFAULT VT_LOAD); break;
    case 's': do_enable_cache(); break;
    case 'd': process_device_tree(device_tree); break;
    // case 'f': print_vt_size(); break;
    case 'g': draw_glasses(); break;
    case 'h': print_performance_counter(); break;
    case 'j': set_input_handler(raw_handler); break;
    // case 'k': vt_fill('X'); break;
    // case 'l': vt_fill(' '); break;

    case 'z': init_irq(); break;
    case 'x': write_timer_compare(1, binary_entry); break;
    case 'c': in_3_seconds(); break;
    case 'b': do_halt(); break;
    case 'm': do_set_mode(); break;

    // TODO: Convert to handler
    case 'W': write_memory(); break;
    // TODO: Convert to handler
    case 'R': read_memory(); break;

    default:
      uart_putc(input);
      break;
  }
}

void single_command_handler(char c) {
  set_input_handler(main_handler);
  command_handler(c);
}

void main_handler(char c) {
  switch(c) {
    case '`': set_input_handler(single_command_handler); break;
    case '~': set_input_handler(command_handler); break;

    case ENTER: uart_puts(EOL); break;
    case BACKSPACE: backspace(); break;

    default:
      uart_putc(c);
      break;
  }
}

void root_handler(char c) {
  if(c == ESCAPE)
    set_input_handler(main_handler);
  else
    input_handler(c);
}

void set_input_handler(void (*handler)(char)) {
  input_handler = handler;
}

void process_input() {
  if(is_polling_enabled)
    uart_getc_pipe(root_handler);
}

void input_loop() {
  while(run_loop) {
    // List "processes" here
    draw_uptime_clock();
    process_input();
  }
}

void memory_range_out(size_t* start, size_t count, void (*out)(unsigned char)) {
  size_t* end = start + count;

  for(size_t* i = start; i < end; i++) {
    print_address(i, out);
    uart_puts(EOL);
  }

  uart_puts(EOL);
}

void print_params(uint32_t r0, uint32_t r1, uint32_t atags) {
  uart_puts("r0:    0x");
  word_to_hex(r0, uart_putc);
  uart_puts(EOL);

  uart_puts("r1:    0x");
  word_to_hex(r1, uart_putc);
  uart_puts(EOL);

  uart_puts("atags: 0x");
  word_to_hex(atags, uart_putc);
  uart_puts(EOL);

  uart_puts(EOL);
}

#if defined(__cplusplus)
extern "C" /* Use C linkage for main. */
#endif

#ifdef AARCH64
// arguments for AArch64
void main(uint64_t dtb_ptr32, uint64_t x1, uint64_t x2, uint64_t x3)
#else
// arguments for AArch32
void main(uint32_t r0, uint32_t r1, uint32_t atags)
#endif
{
  // Initialize UART for Raspberry Pi
  uart_init();

  uart_puts(EOL "=== Hello, Kitsune ===" EOL EOL);

  print_params(r0, r1, atags);

  device_tree = (void*) atags;

  fb_test_2();
  draw_logo();

  draw_aki();

  delay(1000000);

  draw_kitsune_text();
  draw_mascot_text();

  draw_clock(0, 0, 0);

  uart_puts(EOL "READY" EOL);

  input_loop();
}
