#!/usr/bin/env python3
"""Reads blocks from the SD card over JTAG, relying entirely on the firmware's setup.

No initialization: the firmware read the card to boot, and leaves the controller's
clock on and the card ready for commands. Only ever sends CMD17, which reads one
block, so this can't change the card. Checks what it read against the card's
known filesystem ID.
"""
import socket
import sys
import time

TCL = ("127.0.0.1", 6666)
EMMC2 = 0xFE340000
BLOCK_COUNT_SIZE, ARGUMENT, COMMAND, RESPONSE, DATA_PORT = 0x04, 0x08, 0x0C, 0x10, 0x20
INTERRUPT_STATUS = 0x30
WRITABLE = {BLOCK_COUNT_SIZE, ARGUMENT, COMMAND, INTERRUPT_STATUS}

BLOCK_BYTES = 512
WORD_BYTES = 4
ONE_BLOCK = 1 << 16 | BLOCK_BYTES
# CMD17, data present, index and CRC checked, 48-bit response, and reading.
READ_SINGLE_BLOCK = 0x113A0010
COMMAND_COMPLETE, TRANSFER_COMPLETE, READ_READY, ANY_ERROR = 1 << 0, 1 << 1, 1 << 5, 1 << 15
CLEAR_ALL = 0xFFFFFFFF
TIMEOUT_SECONDS = 5

MBR_SIGNATURE_AT = 510
MBR_SIGNATURE = b"\x55\xaa"
FIRST_PARTITION_START_AT = 446 + 8
FAT32_VOLUME_ID_AT = 0x43
EXPECTED_VOLUME_ID = "AE82-4BC1"

sock = socket.create_connection(TCL, timeout=20)


def command(text):
    sock.sendall(text.encode() + b"\x1a")
    data = b""
    while b"\x1a" not in data:
        chunk = sock.recv(65536)
        if not chunk:
            sys.exit("openocd closed the connection")
        data += chunk
    return data.split(b"\x1a")[0].decode(errors="replace").strip()


def read(offset):
    reply = command(f"mdw {EMMC2 + offset:#x} 1")
    if ":" not in reply:
        raise RuntimeError(f"can't read {offset:#x}: {reply}")
    return int(reply.split(":")[1].strip(), 16)


def write(offset, value):
    assert offset in WRITABLE, hex(offset)
    command(f"mww {EMMC2 + offset:#x} {value:#x}")


def wait_for(flag, name):
    deadline = time.monotonic() + TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        status = read(INTERRUPT_STATUS)
        if status & ANY_ERROR:
            raise RuntimeError(f"error waiting for {name}: status {status:#010x}")
        if status & flag:
            write(INTERRUPT_STATUS, flag)
            return
    raise RuntimeError(f"timed out waiting for {name}: status {read(INTERRUPT_STATUS):#010x}")


def read_block(block):
    write(INTERRUPT_STATUS, CLEAR_ALL)
    write(BLOCK_COUNT_SIZE, ONE_BLOCK)
    write(ARGUMENT, block)
    write(COMMAND, READ_SINGLE_BLOCK)
    wait_for(COMMAND_COMPLETE, "the command")
    response = read(RESPONSE)
    wait_for(READ_READY, "data")
    # One word at a time, as the port stays at one address.
    data = b"".join(read(DATA_PORT).to_bytes(WORD_BYTES, "little") for _ in range(BLOCK_BYTES // WORD_BYTES))
    wait_for(TRANSFER_COMPLETE, "the end of the transfer")
    return response, data


command("targets bcm2711.cpu0")
was_running = command("bcm2711.cpu0 curstate") == "running"
command("halt 2000")
if command("bcm2711.cpu0 curstate") != "halted":
    sys.exit("core 0 did not halt")
try:
    start = time.monotonic()
    response, mbr = read_block(0)
    print(f"block 0: card status {response:#010x}, in {time.monotonic() - start:.1f}s over JTAG")
    print("  first bytes", mbr[:16].hex(" "), " signature", mbr[MBR_SIGNATURE_AT:].hex(" "))
    if mbr[MBR_SIGNATURE_AT:] != MBR_SIGNATURE:
        sys.exit("FAIL: block 0 has no MBR signature")

    partition = int.from_bytes(mbr[FIRST_PARTITION_START_AT:FIRST_PARTITION_START_AT + WORD_BYTES], "little")
    response, boot = read_block(partition)
    volume = boot[FAT32_VOLUME_ID_AT:FAT32_VOLUME_ID_AT + WORD_BYTES]
    found = f"{int.from_bytes(volume[2:], 'little'):04X}-{int.from_bytes(volume[:2], 'little'):04X}"
    print(f"block {partition}: first partition's start, card status {response:#010x}, volume ID {found}")
    print("PASS" if found == EXPECTED_VOLUME_ID else f"FAIL: expected {EXPECTED_VOLUME_ID}")
    sys.exit(0 if found == EXPECTED_VOLUME_ID else 1)
finally:
    if was_running:
        command("resume")
