//! Throwaway kernel module: copies the images from RAM to raw blocks on the card, where
//! `card::image_area` says. See run.py in this folder.
//!
//! Only writes inside that area, and only if it's blank, or already holds images. It
//! must run on a boot that got its images from the image file, and it checks they're
//! there. It reads the whole area first, which also shows the card is big enough.

use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};
use core::slice;

use crate::board::{self, read_blocks, BLOCK_BYTES};
use crate::card;

const MAGIC: &[u8] = b"KITSASS2";
const COUNT_AT: usize = 8;
const TABLE_AT: usize = 12;
const ENTRY_BYTES: usize = 8;
const WORD_BYTES: usize = 4;
const REGION: usize = 0x20_0000;
const REGION_BYTES: usize = 6 * 1024 * 1024;
// Unused RAM, for what's on the card.
const CHECK: usize = 0x1000_0000;

const EMMC2: usize = 0xFE34_0000;
const BLOCK_COUNT_SIZE: usize = 0x04;
const ARGUMENT: usize = 0x08;
const COMMAND: usize = 0x0C;
const DATA_PORT: usize = 0x20;
const INTERRUPT_STATUS: usize = 0x30;
const BLOCK_COUNT_SHIFT: u32 = 16;
/// CMD25: data present, index and CRC checked, 48-bit response, writing many blocks, a
/// block count and automatic CMD12.
const WRITE_MANY: u32 = 0x193A_0026;
const COMMAND_COMPLETE: u32 = 1 << 0;
const TRANSFER_COMPLETE: u32 = 1 << 1;
const WRITE_READY: u32 = 1 << 4;
const ANY_ERROR: u32 = 1 << 15;
const CLEAR_ALL: u32 = 0xFFFF_FFFF;
const TIMEOUT_MICROSECONDS: u32 = 2_000_000;

/// Places to look at before writing, each read for as long as the area itself.
const SURVEY_GIB: [u32; 6] = [2, 3, 4, 5, 6, 7];
const BLOCKS_PER_GIB: u32 = 2 * 1024 * 1024;
const SAMPLE_BYTES: usize = 32;

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

fn write_many(first: u32, data: &[u8]) -> Result<(), u32> {
    write_register(INTERRUPT_STATUS, CLEAR_ALL);
    write_register(BLOCK_COUNT_SIZE, ((data.len() / BLOCK_BYTES) as u32) << BLOCK_COUNT_SHIFT | BLOCK_BYTES as u32);
    write_register(ARGUMENT, first);
    write_register(COMMAND, WRITE_MANY);
    wait_for(COMMAND_COMPLETE)?;
    for block in data.chunks_exact(BLOCK_BYTES) {
        wait_for(WRITE_READY)?;
        for word in block.chunks_exact(WORD_BYTES) {
            write_register(DATA_PORT, u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
        }
    }
    wait_for(TRANSFER_COMPLETE)
}

fn word(data: &[u8], at: usize) -> usize {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as usize
}

fn experiment() -> Result<(), &'static str> {
    // SAFETY: the image region and unused RAM. The kernel isn't using its slices of the
    // region now, as this runs in place of its main loop, and it's only read here.
    let (region, check) = unsafe {
        (slice::from_raw_parts(REGION as *const u8, REGION_BYTES), slice::from_raw_parts_mut(CHECK as *mut u8, REGION_BYTES))
    };
    if &region[..MAGIC.len()] != MAGIC {
        return Err("no images in RAM to copy, so wrote nothing");
    }
    let count = word(region, COUNT_AT);
    let mut used = 0;
    for slot in 0..count {
        let entry = TABLE_AT + slot * ENTRY_BYTES;
        used = used.max(word(region, entry) + word(region, entry + WORD_BYTES));
    }
    if used == 0 || used > REGION_BYTES {
        return Err("the image table looks wrong, so wrote nothing");
    }
    let bytes = used.next_multiple_of(BLOCK_BYTES);
    println!("{count} images, {} KB", bytes / 1024);

    for gibibytes in SURVEY_GIB {
        let block = gibibytes * BLOCKS_PER_GIB;
        match read_blocks(block, check) {
            Ok(()) => {
                let nonzero = check.iter().filter(|byte| **byte != 0).count();
                let first = check.iter().position(|byte| *byte != 0);
                let last = check.iter().rposition(|byte| *byte != 0);
                println!("{gibibytes} GiB: {nonzero} nonzero, first at {first:?}, last at {last:?}");
                if let Some(at) = first {
                    let start = at & !(SAMPLE_BYTES - 1);
                    println!("  {:02x?}", &check[start..start + SAMPLE_BYTES]);
                }
            }
            Err(error) => println!("{gibibytes} GiB: {error}"),
        }
    }

    let area = card::image_area().ok_or("couldn't find the area, so wrote nothing")?;
    read_blocks(area, check).map_err(|_| "couldn't read the whole area, so wrote nothing")?;
    let nonzero = check.iter().filter(|byte| **byte != 0).count();
    println!("area at block {area}, {} MB of it reads fine, with {nonzero} nonzero bytes", REGION_BYTES / (1024 * 1024));
    if nonzero != 0 && &check[..MAGIC.len()] != MAGIC {
        return Err("the area holds something else, so wrote nothing");
    }

    let start = board::microseconds();
    write_many(area, &region[..bytes]).map_err(|_| "THE WRITE FAILED")?;
    println!("wrote it in {} ms", board::microseconds().wrapping_sub(start) / 1000);
    read_blocks(area, &mut check[..bytes]).map_err(|_| "couldn't read it back")?;
    println!("read back: {}", if check[..bytes] == region[..bytes] { "matches" } else { "DIFFERENT" });
    Ok(())
}

pub fn run(_framebuffer: &crate::video::Framebuffer) -> ! {
    println!("=== copying images");
    if let Err(problem) = experiment() {
        println!("STOPPED: {problem}");
    }
    println!("DONE");
    loop {
        // SAFETY: only waits for an event.
        unsafe { asm!("wfe", options(nostack)) };
    }
}
