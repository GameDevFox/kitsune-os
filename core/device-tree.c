#include "arm.h"
#include "convert.h"
#include "device-tree.h"
#include "output.h"
#include "uart.h"

enum DeviceTreeToken {
  BEGIN_NODE = 0x00000001,
  END_NODE   = 0x00000002,
  PROP       = 0x00000003,
  NOP        = 0x00000004,
  END        = 0x00000009,
};

#define DEVICE_TREE_MAGIC 0xedfe0dd0

void print_device_tree_header(struct DeviceTreeHeader* header) {
  uart_puts("DEVICE TREE:" EOL);

  word_to_hex(swap_bytes(header->magic), uart_putc); uart_puts(" Magic" EOL);
  word_to_hex(swap_bytes(header->total_size), uart_putc); uart_puts(" Total Size" EOL);

  word_to_hex(swap_bytes(header->struct_offset), uart_putc); uart_puts(" Struct Offset" EOL);
  word_to_hex(swap_bytes(header->strings_offset), uart_putc); uart_puts(" Strings Offset" EOL);
  word_to_hex(swap_bytes(header->memory_reservation_offset), uart_putc); uart_puts(" Mem Reservation Offset" EOL);

  word_to_hex(swap_bytes(header->version), uart_putc); uart_puts(" Version"); uart_puts(EOL);
  word_to_hex(swap_bytes(header->last_compatible_version), uart_putc); uart_puts(" Last Compatible Version" EOL);

  word_to_hex(swap_bytes(header->boot_cpu_physical_id), uart_putc); uart_puts(" Boot CPU Physical ID" EOL);

  word_to_hex(swap_bytes(header->strings_size), uart_putc); uart_puts(" Strings Size" EOL);
  word_to_hex(swap_bytes(header->struct_size), uart_putc); uart_puts(" Struct Size" EOL);

  uart_puts(EOL);
}

void print_indent(uint32_t count) {
  while(count--) {
    uart_putc(' ');
  }
}

void process_device_tree(void* device_tree) {
  struct DeviceTreeHeader* header = device_tree;

  if(DEVICE_TREE_MAGIC != header->magic) {
    uart_puts("ERROR: Won't process device tree. Bad magic: 0x");
    word_to_hex(swap_bytes(header->magic), uart_putc);
    uart_puts(" != 0x");
    word_to_hex(swap_bytes(DEVICE_TREE_MAGIC), uart_putc);
    uart_puts(EOL);
    return;
  }

  // print_device_tree_header(header);

  char* c;

  uint32_t* dt_struct = device_tree + swap_bytes(header->struct_offset);
  char* strings = device_tree + swap_bytes(header->strings_offset);

  // uart_puts("STRUCT: "); word_to_hex((uint32_t) dt_struct, uart_putc); uart_puts(EOL);
  // uart_puts("STRING: "); word_to_hex((uint32_t) strings, uart_putc); uart_puts(EOL);

  uint32_t done = 0;
  uint32_t indent = 0;

  while(1) {
    // word_to_hex(dt_struct, uart_putc); uart_puts(" ");
    enum DeviceTreeToken token = swap_bytes(*dt_struct++);

    // uart_putc('#');
    // word_to_hex(token, uart_putc); uart_putc(' ');

    switch(token) {
      case BEGIN_NODE:
        uart_puts(EOL);
        print_indent(indent);

        c = (char*) dt_struct;
        while(*c != 0) {
          uart_putc(*c++);
        }
        if(c != (char*) dt_struct)
          uart_putc(' ');

        c++;
        uart_puts("{" EOL);

        indent += 2;

        // Align to 32 bits
        dt_struct = (uint32_t*) (c + (3 - ((uint32_t) c - 1) % 4));

        continue;
      case END_NODE:
        indent -= 2;

        print_indent(indent);
        uart_puts("}" EOL);

        continue;
      case PROP:
        print_indent(indent);

        // uart_puts("PROP" EOL);
        uint32_t len = swap_bytes(*dt_struct++);
        uint32_t nameoff = swap_bytes(*dt_struct++);

        uart_puts(strings + nameoff);
        uart_puts(" = ");

        c = (char*) dt_struct;
        char* strEnd = (char*) dt_struct + len;
        for(; c < strEnd; c++) {
          uart_putc(*c);
        }

        // Align to 32 bits
        dt_struct = (uint32_t*) (c + (3 - ((uint32_t) c - 1) % 4));

        uart_puts(EOL);

        continue;
      case NOP:
        // uart_puts("NOP" EOL);
        break;
      case END:
        uart_puts("END" EOL);
        done = 1;
        break;
      default:
        uart_puts("ERROR: Invalid token encountered: ");
        word_to_hex(token, uart_putc);
        uart_puts(EOL);

        done = 1;
        break;
    }

    if(done)
      break;
  }

  uart_puts(EOL);
}
