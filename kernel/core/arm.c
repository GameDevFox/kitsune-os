#include <stddef.h>
#include <stdint.h>

#define ASM_B_TEMPLATE 0xea000000

uint32_t asm_b(int32_t offset) {
  if(offset < 0) {
    offset &= (1 << 24) - 1;
    offset |= (1 << 23);
  }

  return ASM_B_TEMPLATE | offset;
}

#define ASM_BX_TEMPLATE 0xe12fff10

uint32_t asm_bx(char reg) {
	return ASM_BX_TEMPLATE | (reg & 0xf);
}

#define ASM_MCR_TEMPLATE 0xee000010                        // Write Co-Processor
#define ASM_MRC_TEMPLATE (ASM_MCR_TEMPLATE | ASM_MRC_FLAG) // Read Co-Processor

size_t asm_mrc_mcr(
  char coproc, char opc1, char rt,
  char cr_n, char cr_m, char opc2, uint32_t flags
) {
  size_t result = ASM_MCR_TEMPLATE;

  result |= coproc << 8;
  result |= opc1 << 21;
  result |= rt << 12;
  result |= cr_n << 16;
  result |= cr_m << 0;
  result |= opc2 << 5;

  result |= flags;

  return result;
}
