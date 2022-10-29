#include "arm.h"
#include "convert.h"
#include "device-tree.h"
#include "output.h"
#include "serial-protocol.h"
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
    sp_putc(' ');
  }
}

void process_device_tree(void* device_tree) {
  struct DeviceTreeHeader* header = device_tree;

  if(DEVICE_TREE_MAGIC != header->magic) {
    sp_puts("ERROR: Won't process device tree. Bad magic: 0x");
    word_to_hex(swap_bytes(header->magic), sp_putc);
    sp_puts(" != 0x");
    word_to_hex(swap_bytes(DEVICE_TREE_MAGIC), sp_putc);
    sp_puts(EOL);
    return;
  }

  // print_device_tree_header(header);

  char* c;

  uint32_t* dt_struct = device_tree + swap_bytes(header->struct_offset);
  char* strings = device_tree + swap_bytes(header->strings_offset);

  // sp_puts("STRUCT: "); word_to_hex((uint32_t) dt_struct, sp_putc); sp_puts(EOL);
  // sp_puts("STRING: "); word_to_hex((uint32_t) strings, sp_putc); sp_puts(EOL);

  uint32_t done = 0;
  uint32_t indent = 0;

  while(1) {
    // word_to_hex(dt_struct, sp_putc); sp_puts(" ");
    enum DeviceTreeToken token = swap_bytes(*dt_struct++);

    // sp_putc('#');
    // word_to_hex(token, sp_putc); sp_putc(' ');

    switch(token) {
      case BEGIN_NODE:
        sp_puts(EOL);
        print_indent(indent);

        c = (char*) dt_struct;
        while(*c != 0) {
          sp_putc(*c++);
        }
        if(c != (char*) dt_struct)
          sp_putc(' ');

        c++;
        sp_puts("{" EOL);

        indent += 2;

        // Align to 32 bits
        dt_struct = (uint32_t*) (c + (3 - ((uint32_t) c - 1) % 4));

        continue;
      case END_NODE:
        indent -= 2;

        print_indent(indent);
        sp_puts("}" EOL);

        continue;
      case PROP:
        print_indent(indent);

        // sp_puts("PROP" EOL);
        uint32_t len = swap_bytes(*dt_struct++);
        uint32_t nameoff = swap_bytes(*dt_struct++);

        sp_puts(strings + nameoff);
        sp_puts(" = ");

        c = (char*) dt_struct;
        char* strEnd = (char*) dt_struct + len;
        for(; c < strEnd; c++) {
          sp_putc(*c);
        }

        // Align to 32 bits
        dt_struct = (uint32_t*) (c + (3 - ((uint32_t) c - 1) % 4));

        sp_puts(EOL);

        continue;
      case NOP:
        // sp_puts("NOP" EOL);
        break;
      case END:
        sp_puts("END" EOL);
        done = 1;
        break;
      default:
        sp_puts("ERROR: Invalid token encountered: ");
        word_to_hex(token, sp_putc);
        sp_puts(EOL);

        done = 1;
        break;
    }

    if(done)
      break;
  }

  sp_puts(EOL);
}
