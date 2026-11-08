# Serial protocol and client APIs

```
kernel  <-- UART 115200 8N1 / QEMU TCP :8081 -->  serial-client  <-- REST :8080, GraphQL :4000 -->  web UI
```

In TCP mode the client is the server: it listens on 8081 and QEMU connects to it. In serial mode it opens the device given on its command line.

Input and output use different formats. Input is unframed bytes interpreted by a small state machine. Output is plain text, with binary frames mixed in.

## Input modes

The kernel starts in **command mode**. Each polled byte goes through `root_handler` (`kernel/core/main.c`).

| Mode | Enter with | Behaviour |
| --- | --- | --- |
| Command (default) | `~` from echo mode | Each byte is a command. Unknown bytes are echoed |
| Echo | `ESC` (`0x1b`) from any mode | Echoes typing, Enter gives CRLF, `0x7f` is backspace |
| One-shot | `` ` `` from echo mode | Runs one command, then returns to echo mode |
| Raw | `j` command | Prints each byte as hex. `ESC` leaves it |

`ESC` is only intercepted between commands. Argument bytes are read directly by the running command, so a `0x1b` in a payload is safe.

## Commands

Arguments are raw bytes, and multi-byte values are little-endian (`u32`), except where marked as ASCII hex digits.

### Request/response

Each of these takes a client-chosen `target` id and replies in a frame with that id.

| Byte | Arguments | Response payload | Purpose |
| --- | --- | --- | --- |
| `R` | `target`, `start:u32`, `length:u32` | The bytes read | Read memory |
| `W` | `target`, `start:u32`, `length:u32`, `data` | The bytes written, then a raw `Writing Bytes Done!` line after the frame | Write memory |
| `` ` `` | `target`, then 5 ASCII hex digits: `coproc opc1 CRn CRm opc2` | `u32` | MRC, read a coprocessor register |
| `~` | as above, then `value:u32` | `u32` returned by the generated function | MCR, write a coprocessor register |
| `3` | `target`, `write:u8`, then `value:u32` if `write` is nonzero | CPSR as `u32` on read; empty on write | Read or write CPSR |

### Scratch register `binary_entry`

| Byte | Arguments | Purpose |
| --- | --- | --- |
| `e` | `value:u32` | Set `binary_entry` directly. The UI's color picker uses this, as `B G R 0xff` |
| `=` | interactive hex; Enter ends | Edit `binary_entry` by typing |
| `w` | 8 ASCII hex digits | Write a word to the address in `binary_entry` |
| `!` | | Execute `binary_entry` as one instruction |
| `x` | | Set timer compare 1 to `binary_entry` |
| `m` | | Change mode to `binary_entry`, then store the new CPSR back into it |

### Drawing

| Byte | Draws |
| --- | --- |
| `0` | Clears the framebuffer |
| `6` / `7` | Rectangle / curve, in the color in `binary_entry` |
| `8` / `g` | Mascot / its glasses |
| `9` | Logo |
| `#` `$` `%` | Aki base / glasses / no glasses |
| `^` `&` `*` | Aki mouth 0 / 1 / 2 |
| `r` / `y` | Animated "KITSUNE" text / animated speech text |

### Diagnostics and faults

| Byte | Purpose |
| --- | --- |
| `1` | Prints `Hello, Kitsune!` |
| `2` | Triggers a software interrupt |
| `4` | Runs the cycle-counter test and prints the results |
| `5` | Executes `.word 0xffffffff`, an undefined instruction |
| `a` | Prints "Red Text". The colors are compiled out, as `TERM_CONTROL_CODES` isn't defined |
| `d` | Prints the device tree |
| `h` / `t` | Prints the performance counter / the timer and performance counter |

### Control and experiments

| Byte | Purpose |
| --- | --- |
| `@` | Stops UART polling. **There is no way back without a reset** |
| `b` | Halts: the main loop exits |
| `j` | Enters raw mode |
| `q` | Toggles IRQ and FIQ masks, then prints the result |
| `s` | Sets SCTLR.C |
| `z` / `c` | Sets up the timer IRQ through the GIC (Pi 4 only) / sets a compare 3 seconds ahead |

## Output frames

Anything outside a frame is plain text for the console. Frames are built by `kernel/core/serial-protocol.c` and parsed by `serial-client/src/output-handler/`.

| Bytes | Meaning |
| --- | --- |
| `FF` | Start a frame. Always starts a new one, even inside another frame |
| `FE 01` | End the frame |
| `FE 02 <id>` | Set the frame's target id |
| `FE 03 <u32>` | Declare a length. **The client parses it, but the kernel never sends it** |
| `FE 00` | A literal `FF` in the payload |
| `FE FE` | A literal `FE` in the payload |

`sp_putc` handles the escaping, so payload bytes must go through `sp_putc`, `sp_write` or `sp_putw`.

A response looks like this, using `R` at `0x8000` for 4 bytes with target 1:

```
sent:     52 01 00 80 00 00 04 00 00 00
received: FF FE 02 01 AC D0 9F E5 FE 01
```

### Targets

The client (`serial-client/src/request.ts`) allocates the lowest free id from 1 to 127 for each request and resolves it when a frame with that id arrives. A frame with target 0, the default, is written to stdout.

- Requests have no timeout. If the kernel never answers, the HTTP request hangs and the id stays taken.
- Frames form a stack on the client, so an inner frame finishes before its parent. The parent resumes afterward.

### Framing hazards

From reading the code; these were not reproduced.

- **`FF` opens a frame wherever it appears in the stream.** Unframed output isn't escaped. If a `uart_putc` emits `FF`, for example the command handler echoing an unknown input byte, the client treats everything after it as frame contents until a matching end.
- **A fault inside `sp_frame` leaves the frame open.** Reading a faulting address with `R` jumps to `input_loop` with no `FE 01`, so later console output is swallowed and the request never resolves.

## REST API (`serial-client/src/rest.ts`, port 8080)

Responses are `{ success: true, ... }`. Binary data comes back in Node's JSON form, `{ type: "Buffer", data: [...] }`. Numeric path values accept `0x` prefixes.

| Route | Purpose |
| --- | --- |
| `GET /hello` | Command `1` |
| `GET /clear` | Command `0` |
| `GET /draw/:name` | A name from `images` in `serial-client/src/api.ts` |
| `GET /memory/:start/length/:length` | Read memory |
| `POST /memory/:start` `{ data: number[] }` | Write memory; returns what was written |
| `GET /coproc-registers` | All register definitions |
| `GET /coproc-registers/:name` | Read a register |
| `POST /coproc-registers/:name` `{ value: "<hex>" }` | Write a register |
| `GET`, `POST /cpsr` `{ value: number[4] }` | Read or write CPSR |
| `POST /color` `{ color: { r, g, b } }` | Sets `binary_entry` |
| `GET /print-device-tree` | Prints to the client's console, not the response |
| `GET /timer` | Prints to the client's console, not the response |
| `GET /kernel-symbol` | `nm` of `kernel/kitsune-qemu.elf`, always the QEMU build even in serial mode |

## GraphQL API (`serial-client/src/graphql.ts`, port 4000)

Queries:

- `coprocRegisterNames`: register definitions
- `memory(start, length)`: read memory
- `coprocRegister(names)`: **a stub**, returning `[]` or `[1, 2, 3, 4]` without touching the kernel

Mutations:

- `clear`, `draw(names)`, `hello`, `printDeviceTree`, `printTimer`, `setColor(color)`: as in REST
- `sendBytes(bytes)`: sends a string's UTF-8 bytes as raw input, so it cannot send bytes above `0x7f` as themselves
- `instructionAbort`: sends command `5`, which is actually an undefined instruction
- `test`, `flash`: **broken** by stale paths

## Adding a command

1. Add a `case` in `command_handler` in `kernel/core/main.c`.
2. If it returns data, read a `target` byte first and reply inside `sp_frame(sp_target(target); ...)`, without printing raw text inside the frame.
3. Add a command builder in `serial-client/src/commands.ts`, plus a method in `api.ts` and a route in `rest.ts`.
4. If it runs during a flash, make it and its callees `__eden`.
