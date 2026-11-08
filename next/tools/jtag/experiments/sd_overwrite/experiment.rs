//! Throwaway kernel module: overwrites `update-test.bin` on the card through the `fat`
//! crate, or checks what an earlier run left there. See run.py in this folder.

use core::arch::asm;

use fat::{Block, Blocks, DeviceError, Volume};

use crate::board;
use crate::card;

const SCRATCH: &str = "update-test.bin";
const CHECK_ONLY_MARKER: &[u8] = b"CHECK";
const FILE_BYTES: usize = 512 * 1024;
const CONTENT_BYTES: usize = 300_000;
const SEED: u8 = 0x5A;
// Unused RAM for the file's contents, as it's too big for the stack.
const CONTENTS: usize = 0x1000_0000;
// Where the card's first partition starts, as `card::report` prints at boot.
const PARTITION_START: u32 = 8192;

fn pattern(index: usize) -> u8 {
    if index < CONTENT_BYTES { ((index % 251) as u8) ^ SEED ^ ((index >> 16) as u8) } else { 0 }
}

struct Reader;

impl Blocks for Reader {
    fn read(&mut self, block: u32, data: &mut Block) -> Result<(), DeviceError> {
        board::read_blocks(PARTITION_START + block, data).map_err(|_| DeviceError)
    }

    fn write(&mut self, _: u32, _: &Block) -> Result<(), DeviceError> {
        Err(DeviceError)
    }
}

/// Reads the raw file through a chain walk that writes nothing, by asking `overwrite` on a
/// device whose writes are ignored: its check then compares the card with the pattern.
struct CheckOnly;

impl Blocks for CheckOnly {
    fn read(&mut self, block: u32, data: &mut Block) -> Result<(), DeviceError> {
        Reader.read(block, data)
    }

    fn write(&mut self, _: u32, _: &Block) -> Result<(), DeviceError> {
        Ok(())
    }
}

pub fn run(_framebuffer: &crate::video::Framebuffer) -> ! {
    println!("=== overwrite test");
    // SAFETY: unused RAM.
    let contents = unsafe { core::slice::from_raw_parts_mut(CONTENTS as *mut u8, CONTENT_BYTES) };
    for (index, byte) in contents.iter_mut().enumerate() {
        *byte = pattern(index);
    }

    println!("refused: {:?}", card::overwrite("kitsune.img", contents).is_err());
    println!("refused: {:?}", card::overwrite("config.txt", contents).is_err());

    let check_only = option_env!("OVERWRITE_CHECK_ONLY").map(str::as_bytes) == Some(CHECK_ONLY_MARKER);
    if check_only {
        let mut device = CheckOnly;
        let result = Volume::open(&mut device).and_then(|volume| {
            let file = volume.find(&mut device, SCRATCH)?;
            volume.overwrite(&mut device, &file, contents)
        });
        println!("after the reboot, the card holds what was written: {result:?}");
    } else {
        let start = board::microseconds();
        let result = card::overwrite(SCRATCH, contents);
        println!("overwrote {FILE_BYTES} bytes, read back and compared, in {} ms: {result:?}", board::microseconds().wrapping_sub(start) / 1000);
    }
    println!("DONE");
    loop {
        // SAFETY: only waits for an event.
        unsafe { asm!("wfe", options(nostack)) };
    }
}
