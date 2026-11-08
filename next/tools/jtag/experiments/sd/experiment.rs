//! Throwaway kernel module: reads the SD card by polling, with no setup of its own,
//! and measures how fast. See run.py in this folder. Only sends read commands.

use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use crate::board;
use crate::video::Framebuffer;

const EMMC2: usize = 0xFE34_0000;
const BLOCK_COUNT_SIZE: usize = 0x04;
const ARGUMENT: usize = 0x08;
const COMMAND: usize = 0x0C;
const RESPONSE: usize = 0x10;
const DATA_PORT: usize = 0x20;
const INTERRUPT_STATUS: usize = 0x30;

const BLOCK_BYTES: u32 = 512;
const WORDS_PER_BLOCK: u32 = BLOCK_BYTES / 4;
/// CMD17, data present, index and CRC checked, 48-bit response, and reading.
const READ_ONE: u32 = 0x113A_0010;
/// CMD18 with the same flags, plus multiple blocks, a block count and automatic CMD12.
const READ_MANY: u32 = 0x123A_0036;

const COMMAND_COMPLETE: u32 = 1 << 0;
const TRANSFER_COMPLETE: u32 = 1 << 1;
const READ_READY: u32 = 1 << 5;
const ANY_ERROR: u32 = 1 << 15;
const CLEAR_ALL: u32 = 0xFFFF_FFFF;
const TIMEOUT_MICROSECONDS: u32 = 1_000_000;

const FNV_START: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

const PARTITION_ENTRY_START: usize = 446 + 8;
const PARTITION_ENTRY_SIZE: usize = 446 + 12;
const SIGNATURE_AT: usize = 510;
const VOLUME_ID_AT: usize = 0x43;

/// 1 MB, then 16 MB for a sustained speed.
const SHORT_BLOCKS: u32 = 2 * 1024;
const LONG_BLOCKS: u32 = 32 * 1024;
const MICROSECONDS_PER_SECOND: u64 = 1_000_000;
const KILOBYTE: u64 = 1024;

fn read(offset: usize) -> u32 {
    // SAFETY: a register of the SD controller, which the firmware left clocked.
    unsafe { read_volatile((EMMC2 + offset) as *const u32) }
}

fn write(offset: usize, value: u32) {
    // SAFETY: as in `read`. This module's only commands are the two that read.
    unsafe { write_volatile((EMMC2 + offset) as *mut u32, value) };
}

/// Waits for an interrupt status flag and clears it, or returns the status on an error.
fn wait_for(flag: u32) -> Result<(), u32> {
    let start = board::microseconds();
    loop {
        let status = read(INTERRUPT_STATUS);
        if status & ANY_ERROR != 0 {
            return Err(status);
        }
        if status & flag != 0 {
            write(INTERRUPT_STATUS, flag);
            return Ok(());
        }
        if board::microseconds().wrapping_sub(start) > TIMEOUT_MICROSECONDS {
            return Err(status);
        }
    }
}

fn start(command: u32, block: u32, count: u32) -> Result<(), u32> {
    write(INTERRUPT_STATUS, CLEAR_ALL);
    write(BLOCK_COUNT_SIZE, count << 16 | BLOCK_BYTES);
    write(ARGUMENT, block);
    write(COMMAND, command);
    wait_for(COMMAND_COMPLETE)
}

/// Hands each word of the transfer's blocks to `use_word`, then waits for the end.
fn receive(blocks: u32, mut use_word: impl FnMut(u32)) -> Result<(), u32> {
    for _ in 0..blocks {
        wait_for(READ_READY)?;
        for _ in 0..WORDS_PER_BLOCK {
            use_word(read(DATA_PORT));
        }
    }
    wait_for(TRANSFER_COMPLETE)
}

fn read_block(block: u32, buffer: &mut [u8; BLOCK_BYTES as usize]) -> Result<(), u32> {
    start(READ_ONE, block, 1)?;
    let mut at = 0;
    receive(1, |word| {
        buffer[at..at + 4].copy_from_slice(&word.to_le_bytes());
        at += 4;
    })
}

fn mix(hash: &mut u64, word: u32) {
    *hash = (*hash ^ u64::from(word)).wrapping_mul(FNV_PRIME);
}

/// Reads with one command per block, and returns a hash of the data.
fn hash_one_at_a_time(first: u32, count: u32) -> Result<u64, u32> {
    let mut hash = FNV_START;
    for block in first..first + count {
        start(READ_ONE, block, 1)?;
        receive(1, |word| mix(&mut hash, word))?;
    }
    Ok(hash)
}

/// Reads with a single command, and returns a hash of the data.
fn hash_all_at_once(first: u32, count: u32) -> Result<u64, u32> {
    let mut hash = FNV_START;
    start(READ_MANY, first, count)?;
    receive(count, |word| mix(&mut hash, word))?;
    Ok(hash)
}

fn report(name: &str, blocks: u32, read_them: impl FnOnce() -> Result<u64, u32>) {
    let begin = board::microseconds();
    let result = read_them();
    let elapsed = u64::from(board::microseconds().wrapping_sub(begin)).max(1);
    match result {
        Ok(hash) => {
            let bytes = u64::from(blocks) * u64::from(BLOCK_BYTES);
            let speed = bytes * MICROSECONDS_PER_SECOND / elapsed / KILOBYTE;
            println!("{name}: {speed} KiB/s, {elapsed} us, hash {hash:016x}");
        }
        Err(status) => println!("{name}: FAILED, status {status:#010x}"),
    }
}

fn experiment() -> Result<(), u32> {
    let mut block = [0u8; BLOCK_BYTES as usize];

    let begin = board::microseconds();
    read_block(0, &mut block)?;
    let elapsed = board::microseconds().wrapping_sub(begin);
    let status = read(RESPONSE);
    println!("--- block 0: {elapsed} us, card status {status:#x}");
    println!("signature {:02x} {:02x}", block[SIGNATURE_AT], block[SIGNATURE_AT + 1]);

    let word_at = |bytes: &[u8; BLOCK_BYTES as usize], at: usize| {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    };
    let partition = word_at(&block, PARTITION_ENTRY_START);
    let size = word_at(&block, PARTITION_ENTRY_SIZE);
    println!("first partition: block {partition}, {size} blocks");

    read_block(partition, &mut block)?;
    let id = word_at(&block, VOLUME_ID_AT);
    println!("volume ID {:04X}-{:04X}", id >> 16, id & 0xFFFF);

    report("1 MB, one at a time", SHORT_BLOCKS, || hash_one_at_a_time(partition, SHORT_BLOCKS));
    report("1 MB, all at once", SHORT_BLOCKS, || hash_all_at_once(partition, SHORT_BLOCKS));
    report("1 MB, one at a time again", SHORT_BLOCKS, || hash_one_at_a_time(partition, SHORT_BLOCKS));
    if size >= LONG_BLOCKS {
        report("16 MB, all at once", LONG_BLOCKS, || hash_all_at_once(partition, LONG_BLOCKS));
        report("16 MB, all at once again", LONG_BLOCKS, || hash_all_at_once(partition, LONG_BLOCKS));
    }
    read_block(partition, &mut block)?;
    println!("afterward: card status {:#x}", read(RESPONSE));
    Ok(())
}

pub fn run(_framebuffer: &Framebuffer) -> ! {
    match experiment() {
        Ok(()) => println!("DONE"),
        Err(status) => println!("FAILED, status {status:#010x}\nDONE"),
    }
    loop {
        // SAFETY: only waits for an event.
        unsafe { asm!("wfe", options(nostack)) };
    }
}
