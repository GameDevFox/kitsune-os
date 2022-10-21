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
#include "mem.h"
#include "output.h"
#include "system-timer.h"
#include "uart.h"
#include "vt.h"

extern size_t binary_entry;

void print_address(size_t* addr, void (*out)(unsigned char)) {
  word_to_hex((uint32_t)addr, out);

  uart_puts(": 0x");

  uint32_t mem = *(uint32_t*)addr;
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

void walk_memory(size_t* start, void (*out)(unsigned char)) {
  uart_puts("== MEMORY WALK ==" EOL);

  size_t* current = start;

  char increment = 1;
  while(true) {
    while(increment) {
      print_address(current, out);
      out('\r');
      out('\n');

      current++;
      increment--;
    }

    char input = uart_getc();
    if(input == '0' || input == 0x1b) // '0' or Escape
      break;

    char hex = char_as_hex(input);
    if(hex == 0xff) {
      increment = 1;
    } else {
      increment = hex;
      uart_puts("--------" EOL);
    }
  }

  uart_puts("== WALK FINISHED ==" EOL);
}

void read_memory() {
  size_t start = uart_getw();
  size_t length = uart_getw();

  for(size_t i = start; i < start + length; i++) {
    char value = *(char*)i;
    uart_putc(value);
  }
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
  uint32_t time = read_timer();
  word_to_hex(time, uart_putc);
  uart_puts(EOL);
}

void call_instruction() {
  // Write instruction to our function
  *(volatile size_t*)(&live_instruction) = binary_entry;

  // Call function and print result
  size_t result = live_fn();
  word_to_hex(result, uart_putc);
  // uart_puts(EOL);
}

void generate_mrc(bool is_mrc) {
  // uart_puts("coproc" EOL);
  char coproc = getb();
  // uart_puts("opc1" EOL);
  char opc1 = getb();
  // uart_puts("Rt" EOL);
  // char Rt = getb();
  // uart_puts("CRn" EOL);
  char cr_n = getb();
  // uart_puts("CRm" EOL);
  char cr_m = getb();
  // uart_puts("opc2" EOL);
  char opc2 = getb();

  char rt = 0;
  uint32_t flags = is_mrc ? ASM_MRC_FLAG : 0;
  binary_entry = asm_mrc_mcr(coproc, opc1, rt, cr_n, cr_m, opc2, flags);
  // word_to_hex(binary_entry, uart_putc);
  call_instruction();
  // uart_puts(EOL);
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

void in_3_seconds() {
  uart_puts("Set time 3 seconds from now" EOL);
  uint32_t t = read_timer();
  write_timer_compare(1, t + (3 * 1000000));
  uart_puts("Done" EOL);
}

void print_cpsr() {
  size_t mode = get_cpsr();
  word_to_hex(mode, uart_putc);
  uart_puts(EOL);
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

void run_counter() {
  size_t count = get_performance_counter();
  count = count / 0x10000000;

  if(count == last_count)
    return;

  if(count < last_count)
    counter++;
  else
    counter += count - last_count;

  last_count = count;

  uart_puts(VT_SAVE);

  vt_set_pos(1, 2);
  uart_puts(VT_BG_RED VT_BLACK);
  write_digits(counter, uart_putc);
  uart_puts(VT_DEFAULT VT_BG_DEFAULT);

  uart_puts(VT_LOAD);
}

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
    case '9': draw_logo(); break;
    case '0': fb_clear(); break;

    case '!': call_instruction(); break;
    case '@': disable_polling(); break;
    case '=': binary_entry_mode(); break;

    case 'q': do_toggle_irq(); break;
    case 'w': write_word(); break;
    case 'r': walk_memory((size_t*)binary_entry, uart_putc); break;
    case 't': print_timer(); break;

    case 'a': uart_puts(VT_SAVE VT_HOME VT_RED "Red Text" EOL VT_DEFAULT VT_LOAD); break;
    case 's': do_enable_cache(); break;
    case 'd': process_device_tree(device_tree); break;
    // case 'f': print_vt_size(); break;
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
  // char* label;

  // char* color = VT_WHITE;
  // char* bg_color = VT_BG_RED;

  // if(handler == main_handler) {
  //   label = "Normal Mode";
  //   color = VT_BLACK;
  //   bg_color = VT_BG_GREEN;
  // }
  // if(handler == single_command_handler)
  //   label = "Single Command Mode";
  // if(handler == command_handler)
  //   label = "Command Mode";
  // if(handler == raw_handler)
  //   label = "Raw Mode";

  // uart_puts(VT_SAVE);

  // struct VT_Size size = vt_get_size();
  // vt_set_pos(1, 1);

  // uart_puts(color);
  // uart_puts(bg_color);
  // uart_puts(" == ");
  // uart_puts(label);
  // uart_puts(" == " EOL VT_DEFAULT VT_BG_DEFAULT);

  // uart_puts(VT_LOAD);

  input_handler = handler;
}

void process_input() {
  if(is_polling_enabled)
    uart_getc_pipe(root_handler);
}

void input_loop() {
  while(run_loop) {
    // List "processes" here
    // run_counter();
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

  draw_logo();

  uart_puts(EOL "READY" EOL);

  input_loop();
}
