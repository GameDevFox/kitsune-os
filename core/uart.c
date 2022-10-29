#include <stdbool.h>
#include <stdint.h>

#include "mem.h"
#include "uart.h"

extern int raspi;

extern uint32_t mmio_base;

#define ONE "#1"

// Memory-Mapped I/O output
static void mmio_write(uint32_t reg, uint32_t data) {
  write(mmio_base + reg, data);
}

// Memory-Mapped I/O input
static uint32_t mmio_read(uint32_t reg) {
  return read(mmio_base + reg);
}

// Loop <delay> times in a way that the compiler won't optimize away
static void delay(uint32_t count) {
  asm volatile(
    "__delay_%=: subs %[count], %[count], " ONE "; bne __delay_%=\n"
      : "=r"(count): [count]"0"(count) : "cc"
  );
}

// A Mailbox message with set clock rate of PL011 to 3MHz tag
volatile unsigned int  __attribute__((aligned(16))) mbox[9] = {
  9 * 4, 0, 0x38002, 12, 8, 2, 3000000, 0 ,0
};

void uart_init() {
  // Disable UART0.
  mmio_write(UART0_CR, 0x00000000);
  // Setup the GPIO pin 14 && 15.

  // Disable pull up/down for all GPIO pins & delay for 150 cycles.
  mmio_write(GPPUD, 0x00000000);
  delay(150);

  // Disable pull up/down for pin 14,15 & delay for 150 cycles.
  mmio_write(GPPUDCLK0, (1 << 14) | (1 << 15));
  delay(150);

  // Write 0 to GPPUDCLK0 to make it take effect.
  mmio_write(GPPUDCLK0, 0x00000000);

  // Clear pending interrupts.
  mmio_write(UART0_ICR, 0x7FF);

  // Set integer & fractional part of baud rate.
  // Divider = UART_CLOCK/(16 * Baud)
  // Fraction part register = (Fractional part * 64) + 0.5
  // Baud = 115200.

  // For Raspi3 and 4 the UART_CLOCK is system-clock dependent by default.
  // Set it to 3Mhz so that we can consistently set the baud rate
  if(raspi >= 3) {
    // UART_CLOCK = 30000000;
    unsigned int r = (((unsigned int)(&mbox) & ~0xF) | 8);
    // wait until we can talk to the VC
    while(mmio_read(MBOX_STATUS) & 0x80000000) {}
    // send our message to property channel and wait for the response
    mmio_write(MBOX_WRITE, r);
    while((mmio_read(MBOX_STATUS) & 0x40000000) || mmio_read(MBOX_READ) != r) {}
  }

  // Divider = 3000000 / (16 * 115200) = 1.627 = ~1.
  mmio_write(UART0_IBRD, 1);
  // Fractional part register = (.627 * 64) + 0.5 = 40.6 = ~40.
  mmio_write(UART0_FBRD, 40);

  // Enable FIFO & 8 bit data transmission (1 stop bit, no parity).
  mmio_write(UART0_LCRH, (1 << 4) | (1 << 5) | (1 << 6));

  // Mask all interrupts.
  mmio_write(UART0_IMSC, (1 << 1) | (1 << 4) | (1 << 5) | (1 << 6) |
                          (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10));

  // Enable UART0, receive & transfer part of UART.
  mmio_write(UART0_CR, (1 << 0) | (1 << 8) | (1 << 9));
}

// OUTPUT
void uart_putc(unsigned char c) {
  // Wait for UART to become ready to transmit.
  while(mmio_read(UART0_FR) & (1 << 5)) {}
  mmio_write(UART0_DR, c);
}

void uart_puts(const char* str) {
  for(uint32_t i = 0; str[i] != '\0'; i++)
    uart_putc((unsigned char) str[i]);
}

void uart_write(const char* str, uint32_t count) {
  for(uint32_t i = 0; i < count; i++)
    uart_putc((unsigned char) str[i]);
}

// INPUT
unsigned char uart_getc() {
  // Wait for UART to have received something.
  while(mmio_read(UART0_FR) & (1 << 4)) {}
  return mmio_read(UART0_DR);
}

void uart_getc_pipe(void out()) {
  bool char_available = !(mmio_read(UART0_FR) & (1 << 4));

  if(char_available) {
    char c = mmio_read(UART0_DR);
    out(c);
  }
}

uint32_t uart_getw() {
  uint32_t result = 0;

  result |= uart_getc() << 0x00;
  result |= uart_getc() << 0x08;
  result |= uart_getc() << 0x10;
  result |= uart_getc() << 0x18;

  return result;
}
