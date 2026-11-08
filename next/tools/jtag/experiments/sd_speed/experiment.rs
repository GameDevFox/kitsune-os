//! Throwaway kernel module: how fast the SD card writes, one block at a time and many.
//! See run.py in this folder.
//!
//! Only writes between `AREA_FIRST` and `AREA_END`, in the gap before the first
//! partition, which the partition table says nothing owns. It saves the area first, and
//! restores and checks it at the end, or says so loudly if it can't.

use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};
use core::slice;

use crate::board::{self, read_blocks, BLOCK_BYTES};

const AREA_FIRST: u32 = 4096;
const AREA_BLOCKS: usize = 2048;
const AREA_END: u32 = AREA_FIRST + AREA_BLOCKS as u32;
const AREA_BYTES: usize = AREA_BLOCKS * BLOCK_BYTES;
// Unused RAM, for what was in the area, the test pattern, and what reads back.
const SAVED: usize = 0x1000_0000;
const PATTERN: usize = 0x1100_0000;
const CHECK: usize = 0x1200_0000;

const PARTITION_START_AT: usize = 446 + 8;
const SIGNATURE_AT: usize = 510;
const SIGNATURE: [u8; 2] = [0x55, 0xAA];
const WORD_BYTES: usize = 4;
const RUNS: usize = 5;

const EMMC2: usize = 0xFE34_0000;
const BLOCK_COUNT_SIZE: usize = 0x04;
const ARGUMENT: usize = 0x08;
const COMMAND: usize = 0x0C;
const DATA_PORT: usize = 0x20;
const INTERRUPT_STATUS: usize = 0x30;
const BLOCK_COUNT_SHIFT: u32 = 16;
/// CMD24, data present, index and CRC checked, 48-bit response, and writing.
const WRITE_ONE: u32 = 0x183A_0000;
/// CMD25 with the same flags, plus multiple blocks, a block count and automatic CMD12.
const WRITE_MANY: u32 = 0x193A_0026;
const COMMAND_COMPLETE: u32 = 1 << 0;
const TRANSFER_COMPLETE: u32 = 1 << 1;
const WRITE_READY: u32 = 1 << 4;
const ANY_ERROR: u32 = 1 << 15;
const CLEAR_ALL: u32 = 0xFFFF_FFFF;
const TIMEOUT_MICROSECONDS: u32 = 2_000_000;

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

/// The only way this experiment writes. Refuses anything outside the area.
fn write_blocks(first: u32, data: &[u8]) -> Result<(), u32> {
    let count = (data.len() / BLOCK_BYTES) as u32;
    assert!(data.len().is_multiple_of(BLOCK_BYTES) && count > 0);
    assert!(first >= AREA_FIRST && first + count <= AREA_END);
    write_register(INTERRUPT_STATUS, CLEAR_ALL);
    write_register(BLOCK_COUNT_SIZE, count << BLOCK_COUNT_SHIFT | BLOCK_BYTES as u32);
    write_register(ARGUMENT, first);
    write_register(COMMAND, if count == 1 { WRITE_ONE } else { WRITE_MANY });
    wait_for(COMMAND_COMPLETE)?;
    for block in data.chunks_exact(BLOCK_BYTES) {
        wait_for(WRITE_READY)?;
        for word in block.chunks_exact(WORD_BYTES) {
            write_register(DATA_PORT, u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
        }
    }
    wait_for(TRANSFER_COMPLETE)
}

fn area(address: usize) -> &'static mut [u8] {
    // SAFETY: unused RAM, mapped by the kernel, and each address is used for one thing.
    unsafe { slice::from_raw_parts_mut(address as *mut u8, AREA_BYTES) }
}

fn report(name: &str, runs: &mut [u32; RUNS], bytes: usize) {
    runs.sort_unstable();
    let median = runs[RUNS / 2];
    let speed = bytes as u64 * 1_000_000 / u64::from(median).max(1) / 1024;
    println!("{name:16} {:>7} us min, {median:>7} us median, {:>7} us max, {speed} KiB/s", runs[0], runs[RUNS - 1]);
}

fn time_writes(name: &str, pattern: &[u8], blocks: usize) -> Result<(), &'static str> {
    let mut runs = [0; RUNS];
    for run in &mut runs {
        let start = board::microseconds();
        write_blocks(AREA_FIRST, &pattern[..blocks * BLOCK_BYTES]).map_err(|_| "a write failed")?;
        *run = board::microseconds().wrapping_sub(start);
    }
    report(name, &mut runs, blocks * BLOCK_BYTES);
    Ok(())
}

fn time_reads(check: &mut [u8]) -> Result<(), &'static str> {
    let mut runs = [0; RUNS];
    for run in &mut runs {
        let start = board::microseconds();
        read_blocks(AREA_FIRST, check).map_err(|_| "a read failed")?;
        *run = board::microseconds().wrapping_sub(start);
    }
    report("read 1 MB", &mut runs, AREA_BYTES);
    Ok(())
}

fn measure(saved: &mut [u8], pattern: &mut [u8], check: &mut [u8]) -> Result<(), &'static str> {
    let mut value: u8 = 0xA5;
    for byte in pattern.iter_mut() {
        *byte = value;
        value = value.wrapping_add(7);
    }
    let _ = saved;
    time_writes("write 1 block", pattern, 1)?;
    time_writes("write 8 blocks", pattern, 8)?;
    time_writes("write 64 blocks", pattern, 64)?;
    time_writes("write 1 MB", pattern, AREA_BLOCKS)?;
    time_reads(check)?;
    println!("1 MB read back: {}", if *check == *pattern { "matches" } else { "DIFFERENT" });
    Ok(())
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
    if partition < AREA_END {
        return Err("the area isn't before the first partition, so wrote nothing");
    }
    let (saved, pattern, check) = (area(SAVED), area(PATTERN), area(CHECK));
    read_blocks(AREA_FIRST, saved).map_err(|_| "couldn't save the area, so wrote nothing")?;
    let used = saved.iter().filter(|byte| **byte != 0).count();
    println!("area {AREA_FIRST} to {AREA_END}, first partition at {partition}, holds {used} nonzero bytes");

    let measured = measure(saved, pattern, check);
    if let Err(problem) = measured {
        println!("PROBLEM: {problem}. Restoring anyway.");
    }
    write_blocks(AREA_FIRST, saved).map_err(|_| "RESTORING FAILED")?;
    read_blocks(AREA_FIRST, check).map_err(|_| "COULDN'T READ THE RESTORED AREA")?;
    println!("area restored: {}", if *check == *saved { "matches" } else { "DIFFERENT" });
    Ok(())
}

pub fn run(_framebuffer: &crate::video::Framebuffer) -> ! {
    println!("=== SD write speed");
    if let Err(problem) = experiment() {
        println!("STOPPED: {problem}");
    }
    println!("DONE");
    loop {
        // SAFETY: only waits for an event.
        unsafe { asm!("wfe", options(nostack)) };
    }
}
