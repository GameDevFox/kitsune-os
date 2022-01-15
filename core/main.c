#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "../arch/arm/asm.h"

#include "arm.h"
#include "convert.h"
#include "fb.h"
#include "gic.h"
#include "input.h"
#include "mem.h"
#include "output.h"
#include "system-timer.h"
#include "vt.h"
#include "uart.h"

extern size_t binaryEntry;

void printAddress(size_t* addr,  void (*out)(unsigned char)) {
	wordToHex((uint32_t) addr, out);

	out(':');
	out(' ');
	out('0');
	out('x');

	uint32_t mem = *(uint32_t*) addr;
	wordToHex(mem, out);
}

void printCycleCounterList() {
	uart_puts("CYCLE COUNTER LIST:" EOL);

	size_t prev = 0;
	for(int i=0; i<8; i++) {
		size_t* address = (void*)0x40 + (i * 4);
		printAddress(address, uart_putc);

		size_t current = *address;
		size_t delta = current - prev;
		prev = current;

		if(i != 0) {
			uart_puts(" :: ");
			wordToHex(delta, uart_putc);
		}

		uart_puts(EOL);
	}
}

void walkMemory(size_t* start, void (*out)(unsigned char)) {
	uart_puts("== MEMORY WALK ==" EOL);

	size_t* current = start;

	char increment = 1;
	while(true) {
		while(increment) {
			printAddress(current, out);
			out('\r');
			out('\n');

			current++;
			increment--;
		}

		char input = uart_getc();
		if(input == '0' || input == 0x1b) // '0' or Escape
			break;

		char hex = charAsHex(input);
		if(hex == 0xff) {
			increment = 1;
		} else {
			increment = hex;
			uart_puts("--------" EOL);
		}
	}

	uart_puts("== WALK FINISHED ==" EOL);
}

void readMemory() {
	size_t size = uart_getw();

	for(size_t i=binaryEntry; i<binaryEntry+size; i++) {
		char value = *(char*)i;
		uart_putc(value);
	}
}

void writeMemory() {
	size_t start = uart_getw();
	size_t size = uart_getw();

	uart_puts("Writing ");
	wordToHex(size, uart_putc);
	uart_puts(" bytes to ");
	wordToHex(start, uart_putc);
	uart_puts(" ...");

	size_t end = start+size;

	for(size_t i=start; i<end; i++) {
		char value = uart_getc();
		*(char*)i = value;
	}

	uart_puts(" Done!" EOL);
}

int runLoop = 1;
bool isPollingEnabled = true;

void backspace() {
	// TODO: Fix this hacky way, there must be a better way
	uart_putc('\b');
	uart_putc(' ');
	uart_putc('\b');
}

void printTimer() {
	uint32_t time = readTimer();
	wordToHex(time, uart_putc);
	uart_puts(EOL);
}

void generateMRC(bool isMRC) {
  uart_puts("coproc" EOL);
  char coproc = getb();
  uart_puts("opc1" EOL);
  char opc1 = getb();
  // uart_puts("Rt" EOL);
  // char Rt = getb();
  uart_puts("CRn" EOL);
  char CRn = getb();
  uart_puts("CRm" EOL);
  char CRm = getb();
  uart_puts("opc2" EOL);
  char opc2 = getb();

  char Rt = 0;
  uint32_t flags = isMRC ? ASM_MRC_FLAG : 0;
  binaryEntry = asm_mrc_mcr(
    coproc, opc1, Rt, CRn, CRm, opc2, flags
  );
  wordToHex(binaryEntry, uart_putc);
  uart_puts(EOL);
}

void writeWord() {
  uart_puts("Enter hex to write to ");
  wordToHex(binaryEntry, uart_putc);
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

  *(uint32_t*)binaryEntry = value;

  uart_puts("Wrote ");
  wordToHex(value, uart_putc);
  uart_puts(EOL);
}

void initIRQ() {
	uart_puts("enableIRQ and routeIRQtoCPUS" EOL);
	enableIRQ(VC_IRQ_TIMER_1);
	routeIRQtoCPUS(VC_IRQ_TIMER_1, 0x01);
	uart_puts("Done" EOL);
}

void in3Seconds() {
  uart_puts("Set time 3 seconds from now" EOL);
  uint32_t t = readTimer();
  writeTimerCompare(1, t + (3 * 1000000));
  uart_puts("Done" EOL);
}

void printCPSR() {
  size_t mode = getCPSR();
  wordToHex(mode, uart_putc);
  uart_puts(EOL);
}

void performanceTest() {
  perfTest();
  printCycleCounterList();
  uart_puts("Done" EOL EOL);
}

void callInstruction() {
  // Write instruction to our function
  *(volatile size_t*)(&liveInstruction) = binaryEntry;

  // Call function and print result
  size_t result = liveFn();
  wordToHex(result, uart_putc);
  uart_puts(EOL);
}

void doHalt() {
  runLoop = 0;
  uart_puts("HALT" EOL);
}

void disablePolling() {
  uart_puts("Disabled..." EOL);
  isPollingEnabled = 0;
}

void doToggleIRQ() {
  size_t intMode = toggleIRQs();
  wordToHex(intMode, uart_putc);
  uart_puts(EOL);
}

void doSetMode() {
  setMode(binaryEntry);
  binaryEntry = getCPSR();
  wordToHex(binaryEntry, uart_putc);
  uart_puts(EOL);
}

void doEnableCache() {
  enable_cache();
  uart_puts("Cache enabled" EOL);
}

void printCursorPos() {
  struct VT_Size pos = VT_getPos();

  uart_puts("Row: ");
  wordToHex(pos.row, uart_putc);
  uart_puts(" Column: ");
  wordToHex(pos.column, uart_putc);
  uart_puts(EOL);
}

void printVTSize() {
  struct VT_Size size = VT_getSize();

  uart_puts("Rows: ");
  wordToHex(size.row, uart_putc);
  uart_puts(" Columns: ");
  wordToHex(size.column, uart_putc);
  uart_puts(EOL);
}

void printPerformanceCounter() {
  uart_puts("Performance: ");
  size_t count = getPerformanceCounter();
  wordToHex(count, uart_putc);
  uart_puts(EOL);
}

void (*charHandler)(char) = NULL;

void input_switch(char);

void rawOutput(char c) {
  if(c == '\e') // ESCAPE
    charHandler = input_switch;
  else
    uart_putc(c);
}

void input_switch(char input) {
  switch(input) {
    case '`': generateMRC(1); break;
    case '~': generateMRC(0); break;

    case '1': uart_puts("Hello, Kitsune!" EOL); break;
    case '2': asm(SWI(1234)); break;
    case '3': printCPSR(); break;
    case '4': performanceTest(); break;
    case '5': asm(".word 0xffffffff"); break;
    case '6': fbTest(); break;
    case '7': fbTest2(); break;
    case '9': drawLogo(); break;
    case '0': fbClear(); break;

    case '!': callInstruction(); break;
    case '@': disablePolling(); break;

    case '=': binaryEntryMode(); break;

    case 'q': doToggleIRQ(); break;
    case 'w': writeWord(); break;

    case 'r': walkMemory((size_t*)binaryEntry, uart_putc); break;
    case 't': printTimer(); break;

    case 'a': uart_puts(VT_SAVE VT_HOME VT_RED "Red Text\r\n" VT_DEFAULT VT_LOAD); break;
    case 's': doEnableCache(); break;
    case 'd': printCursorPos(); break;
    case 'f': printVTSize(); break;
    case 'g': VT_fill(' '); break;
    case 'h': VT_fill('X'); break;
    case 'j': charHandler = rawOutput; break;
    case 'k': printPerformanceCounter(); break;

    case 'z': initIRQ(); break;
    case 'x': writeTimerCompare(1, binaryEntry); break;
    case 'c': in3Seconds(); break;

    case 'b': doHalt(); break;

    case 'm': doSetMode(); break;

    case 'W': writeMemory(); break;
    case 'R': readMemory(); break;

    // ENTER
    case 0x0d: uart_puts("\r\n"); break;
    // DEL
    case 0x7f: backspace(); break;

    default:
      uart_putc(input);
      break;
  }
}

size_t lastCount = 0;
size_t counter = 0;

void runCounter() {
  size_t count = getPerformanceCounter();
  count = count / 0x10000000;

  if(count == lastCount)
    return;

  if(count < lastCount)
    counter++;
  else
    counter += count - lastCount;

  lastCount = count;

  uart_puts(VT_SAVE);

  VT_setPos(1, 2);
  uart_puts(VT_BG_RED VT_BLACK);
  write_digits(counter, uart_putc);
  uart_puts(VT_DEFAULT VT_BG_DEFAULT);

  uart_puts(VT_LOAD);
}

#define ENTER 0x0d
#define ESCAPE 0x1b
#define BACKSPACE 0x7f

void mainHandler(char);
void setInputHandler(void (*handler)(char));

void (*inputHandler)(char) = mainHandler;

void rawHandler(char c) {
  byteToHex(c, uart_putc);
  uart_putc(' ');
}

void commandHandler(char input) {
  switch(input) {
    case '`': generateMRC(1); break;
    case '~': generateMRC(0); break;

    case '1': uart_puts("Hello, Kitsune!" EOL); break;
    case '2': asm(SWI(1234)); break;
    case '3': printCPSR(); break;
    case '4': performanceTest(); break;
    case '5': asm(".word 0xffffffff"); break;
    case '6': fbTest(); break;
    case '7': fbTest2(); break;
    case '9': drawLogo(); break;
    case '0': fbClear(); break;

    case '!': callInstruction(); break;
    case '@': disablePolling(); break;
    case '=': binaryEntryMode(); break;

    case 'q': doToggleIRQ(); break;
    case 'w': writeWord(); break;
    case 'r': walkMemory((size_t*)binaryEntry, uart_putc); break;
    case 't': printTimer(); break;

    case 'a': uart_puts(VT_SAVE VT_HOME VT_RED "Red Text" EOL VT_DEFAULT VT_LOAD); break;
    case 's': doEnableCache(); break;
    case 'd': printCursorPos(); break;
    case 'f': printVTSize(); break;
    case 'h': VT_fill('X'); break;
    case 'j': setInputHandler(rawHandler); break;
    case 'k': printPerformanceCounter(); break;
    case 'l': VT_fill(' '); break;

    case 'z': initIRQ(); break;
    case 'x': writeTimerCompare(1, binaryEntry); break;
    case 'c': in3Seconds(); break;
    case 'b': doHalt(); break;
    case 'm': doSetMode(); break;

    case 'W': writeMemory(); break;
    case 'R': readMemory(); break;

    default:
      uart_putc(input);
      break;
  }
}

void singleCommandHandler(char c) {
  setInputHandler(mainHandler);
  commandHandler(c);
}

void mainHandler(char c) {
  switch(c) {
    case '`': setInputHandler(singleCommandHandler); break;
    case '~': setInputHandler(commandHandler); break;

    case ENTER: uart_puts(EOL); break;
    case BACKSPACE: backspace(); break;

    default:
      uart_putc(c);
      break;
  }
}

void rootHandler(char c) {
  if(c == ESCAPE)
    setInputHandler(mainHandler);
  else
    inputHandler(c);
}

void setInputHandler(void (*handler)(char)) {
  char* label;

  char* color = VT_WHITE;
  char* bgColor = VT_BG_RED;

  if(handler == mainHandler) {
    label = "Normal Mode";
    color = VT_BLACK;
    bgColor = VT_BG_GREEN;
  }
  if(handler == singleCommandHandler)
    label = "Single Command Mode";
  if(handler == commandHandler)
    label = "Command Mode";
  if(handler == rawHandler)
    label = "Raw Mode";

  uart_puts(VT_SAVE);

  // struct VT_Size size = VT_getSize();
  VT_setPos(1, 1);

  uart_puts(color);
  uart_puts(bgColor);
  uart_puts(" == ");
  uart_puts(label);
  uart_puts(" == " EOL VT_DEFAULT VT_BG_DEFAULT);

  uart_puts(VT_LOAD);

  inputHandler = handler;
}

void processInput() {
  if(isPollingEnabled)
    uart_getc_pipe(rootHandler);
}

void input_loop() {
  while (runLoop) {
    // List "processes" here
    runCounter();
    processInput();
  }
}

void memoryRangeOut(size_t* start, size_t count, void (*out)(unsigned char)) {
  size_t* end = start + count;

  for(size_t* i = start; i < end; i++) {
    printAddress(i, out);
    uart_puts(EOL);
  }

  uart_puts(EOL);
}

void printParams(uint32_t r0, uint32_t r1, uint32_t atags) {
  uart_puts("r0:    0x");
  wordToHex(r0, uart_putc);
  uart_puts(EOL);

  uart_puts("r1:    0x");
  wordToHex(r1, uart_putc);
  uart_puts(EOL);

  uart_puts("atags: 0x");
  wordToHex(atags, uart_putc);
  uart_puts(EOL);

  uart_puts(EOL);
}

#if defined(__cplusplus)
extern "C" /* Use C linkage for kernel_main. */
#endif

#ifdef AARCH64
// arguments for AArch64
void kernel_main(uint64_t dtb_ptr32, uint64_t x1, uint64_t x2, uint64_t x3)
#else
// arguments for AArch32
void kernel_main(uint32_t r0, uint32_t r1, uint32_t atags)
#endif
{
  // Initialize UART for Raspberry Pi
  uart_init();

  uart_puts("Hello, Kitsune!" EOL);
  uart_puts(EOL);

  printParams(r0, r1, atags);

  // uart_puts("DEVICE TREE:" EOL);
  // memoryRangeOut((size_t*)atags, 32, uart_putc);

  drawLogo();

  uart_puts("READY" EOL);

  input_loop();
}
