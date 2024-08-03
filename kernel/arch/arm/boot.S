#include "mode.h"

// To keep this in the first portion of the binary.
.section ".text.boot"

// Entry point for the kernel.
// r15 -> should begin execution at 0x8000.
// r0 -> 0x00000000
// r1 -> 0x00000c42 - machine id
// r2 -> 0x00000100 - start of ATAGS
// preserve these registers as argument for kernel_main
.globl _start
_start:
  // Setup the stack and save r0, r1, r2
  ldr sp, =_start

  // replace r2 with "_binary___dts_4_model_b_dtb_start" if zero
  cmp r2, #0
  ldreq r2, =_binary___dts_4_model_b_dtb_start

  push {r0-r2}

disable_extra_cores:
  // Shut off extra cores
  mrc p15, 0, r5, c0, c0, 5
  and r5, r5, #3
  cmp r5, #0
  bne halt

// misc:
//   mov r7, #0
//   mcr p15, 4, r7, c1, c1, 0 // HCR

check_for_hyp_mode:
  bl get_mode
  cmp r0, #HYP_MODE
  bleq switch_hyp_to_svc

// enable_high_exception_vectors:
//   mrc p15, 0, r0, c1, c0, 0
//   orr r0, #0x00002000
//   mcr p15, 0, r0, c1, c0, 0

//   mov r2, #0xff000000
//   orr r2, #0x00ff0000

set_vbar:
  // Set VBAR to jump table
  ldr r0, =exception_vector
  mcr p15, 0, r0, c12, c0, 0

set_exception_stack_pointers:
  ldr r0, =__exception_stack_undefined_instruction
  msr sp_und, r0
  ldr r0, =__exception_stack_data_abort
  msr sp_abt, r0
  ldr r0, =__exception_stack_irq
  msr sp_irq, r0
  ldr r0, =__exception_stack_fiq
  msr sp_fiq, r0

enable_cycle_counter:
  mrc p15, 0, r0, c9, c12, 1
  orr r0, r0, #0x80000000 // Enable PMCNTENSET.C
  mcr p15, 0, r0, c9, c12, 1

  mrc p15, 0, r0, c9, c12, 0
  orr r0, r0, #0x1 // Enable PMCR.E
  mcr p15, 0, r0, c9, c12, 0

  mov r0, #0x1f;
  mcr p15, 0, r0, c9, c12, 5 // Set PMSELR.SEL to CCNT (#31)

zero_out_bss:
  ldr r4, =__bss_start
  ldr r9, =__bss_end
  mov r5, #0
  mov r6, #0
  mov r7, #0
  mov r8, #0
  b       2f
1:
  // store multiple at r4.
  stmia r4!, {r5-r8}
2:
  // If we are still below bss_end, loop.
  cmp r4, r9
  blo 1b

  bl perf_test

enter_kernel:
  // Pop the three registers (r0, r1, r2) we pushed earlier
  pop {r0-r2}

  // GDB Trap
// 3:
//   b 3b

  // Call main
  ldr r3, =main
  blx r3

.globl halt
halt:
  wfi
  b halt
