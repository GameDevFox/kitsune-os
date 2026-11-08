//! Throwaway kernel module: the first write to the SD card. See run.py in this folder.
//!
//! Only ever writes to `SCRATCH_BLOCK`, in the gap before the first partition, which
//! the partition table says nothing owns. It saves what's there, writes a pattern,
//! reads it back, then puts the original back and reads that back too. It stops
//! before writing anything if any read fails or the gap isn't where it should be.

use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

use crate::board::{self, read_blocks, BLOCK_BYTES};

const SCRATCH_BLOCK: u32 = 8000;
const PARTITION_START_AT: usize = 446 + 8;
const SIGNATURE_AT: usize = 510;
const SIGNATURE: [u8; 2] = [0x55, 0xAA];
const WORD_BYTES: usize = 4;

const EMMC2: usize = 0xFE34_0000;
const BLOCK_COUNT_SIZE: usize = 0x04;
const ARGUMENT: usize = 0x08;
const COMMAND: usize = 0x0C;
const RESPONSE: usize = 0x10;
const DATA_PORT: usize = 0x20;
const INTERRUPT_STATUS: usize = 0x30;
const ONE_BLOCK: u32 = 1 << 16 | BLOCK_BYTES as u32;
/// CMD24, data present, index and CRC checked, 48-bit response, and writing.
const WRITE_ONE: u32 = 0x183A_0000;
const COMMAND_COMPLETE: u32 = 1 << 0;
const TRANSFER_COMPLETE: u32 = 1 << 1;
const WRITE_READY: u32 = 1 << 4;
const ANY_ERROR: u32 = 1 << 15;
const CLEAR_ALL: u32 = 0xFFFF_FFFF;
const TIMEOUT_MICROSECONDS: u32 = 1_000_000;

const PATTERN_START: u8 = 0xA5;
const PATTERN_STEP: u8 = 7;

fn read_register(offset: usize) -> u32 {
    // SAFETY: a register of the SD controller, which the firmware left clocked.
    unsafe { read_volatile((EMMC2 + offset) as *const u32) }
}

fn write_register(offset: usize, value: u32) {
    // SAFETY: as in `read_register`.
    unsafe { write_volatile((EMMC2 + offset) as *mut u32, value) };
}

fn wait_for(flag: u32) -> Result<(), u32> {
    let start = board::microseconds();
    loop {
        let status = read_register(INTERRUPT_STATUS);
        if status & ANY_ERROR != 0 {
            return Err(status);
        }
        if status & flag != 0 {
            write_register(INTERRUPT_STATUS, flag);
            return Ok(());
        }
        if board::microseconds().wrapping_sub(start) > TIMEOUT_MICROSECONDS {
            return Err(status);
        }
    }
}

/// The only write in this experiment, and it can only reach `SCRATCH_BLOCK`.
fn write_scratch_block(data: &[u8; BLOCK_BYTES]) -> Result<u32, u32> {
    write_register(INTERRUPT_STATUS, CLEAR_ALL);
    write_register(BLOCK_COUNT_SIZE, ONE_BLOCK);
    write_register(ARGUMENT, SCRATCH_BLOCK);
    write_register(COMMAND, WRITE_ONE);
    wait_for(COMMAND_COMPLETE)?;
    let response = read_register(RESPONSE);
    wait_for(WRITE_READY)?;
    for word in data.chunks_exact(WORD_BYTES) {
        write_register(DATA_PORT, u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
    }
    wait_for(TRANSFER_COMPLETE)?;
    Ok(response)
}

fn experiment() -> Result<(), &'static str> {
    let mut mbr = [0; BLOCK_BYTES];
    read_blocks(0, &mut mbr).map_err(|_| "couldn't read block 0, so wrote nothing")?;
    if mbr[SIGNATURE_AT..SIGNATURE_AT + SIGNATURE.len()] != SIGNATURE {
        return Err("no partition table, so wrote nothing");
    }
    let mut start = [0; WORD_BYTES];
    start.copy_from_slice(&mbr[PARTITION_START_AT..PARTITION_START_AT + WORD_BYTES]);
    let partition = u32::from_le_bytes(start);
    println!("first partition at block {partition}, scratch block {SCRATCH_BLOCK}");
    if partition <= SCRATCH_BLOCK {
        return Err("the scratch block isn't before the first partition, so wrote nothing");
    }

    let mut original = [0; BLOCK_BYTES];
    read_blocks(SCRATCH_BLOCK, &mut original).map_err(|_| "couldn't read the scratch block, so wrote nothing")?;
    let used = original.iter().filter(|byte| **byte != 0).count();
    println!("scratch block holds {used} nonzero bytes");

    let mut pattern = [0; BLOCK_BYTES];
    let mut value = PATTERN_START;
    for byte in &mut pattern {
        *byte = value;
        value = value.wrapping_add(PATTERN_STEP);
    }

    let begin = board::microseconds();
    let status = write_scratch_block(&pattern).map_err(|_| "the write failed")?;
    println!("wrote it in {} us, card status {status:#x}", board::microseconds().wrapping_sub(begin));

    let mut check = [0; BLOCK_BYTES];
    read_blocks(SCRATCH_BLOCK, &mut check).map_err(|_| "couldn't read the pattern back")?;
    println!("pattern read back: {}", if check == pattern { "matches" } else { "DIFFERENT" });

    write_scratch_block(&original).map_err(|_| "restoring the original failed")?;
    read_blocks(SCRATCH_BLOCK, &mut check).map_err(|_| "couldn't read the original back")?;
    println!("original restored: {}", if check == original { "matches" } else { "DIFFERENT" });
    Ok(())
}

pub fn run(_framebuffer: &crate::video::Framebuffer) -> ! {
    println!("=== writing one block");
    if let Err(problem) = experiment() {
        println!("STOPPED: {problem}");
    }
    println!("DONE");
    loop {
        // SAFETY: only waits for an event.
        unsafe { asm!("wfe", options(nostack)) };
    }
}
