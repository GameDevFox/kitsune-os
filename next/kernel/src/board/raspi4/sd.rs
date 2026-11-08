//! Reading and writing the SD card through the EMMC2 controller, by polling.
//!
//! There's no setup: the firmware read the card to boot, and leaves the controller's
//! clock on and the card selected and idle. That also means this needs the real
//! firmware, so it times out in QEMU. Blocks are numbered, which assumes an SDHC or
//! larger card, as older ones address by byte. See "Reading the SD card" in
//! `docs/hardware-notes.md`. All registers are read and written 32 bits at a time, as
//! Linux's driver does on this chip.

use core::fmt;
use core::ptr::{read_volatile, write_volatile};

use super::{microseconds, PERIPHERAL_BASE};

const EMMC2: usize = PERIPHERAL_BASE + 0x34_0000;
const BLOCK_COUNT_SIZE: usize = 0x04;
const ARGUMENT: usize = 0x08;
const COMMAND: usize = 0x0C;
const DATA_PORT: usize = 0x20;
const INTERRUPT_STATUS: usize = 0x30;

pub const BLOCK_BYTES: usize = 512;
const WORD_BYTES: usize = 4;
const BLOCK_COUNT_SHIFT: u32 = 16;

/// CMD17, data present, index and CRC checked, 48-bit response, and reading.
const READ_ONE: u32 = 0x113A_0010;
/// CMD18 with the same flags, plus multiple blocks, a block count and automatic CMD12.
const READ_MANY: u32 = 0x123A_0036;

/// CMD24, data present, index and CRC checked, 48-bit response, and writing.
const WRITE_ONE: u32 = 0x183A_0000;

const COMMAND_COMPLETE: u32 = 1 << 0;
const TRANSFER_COMPLETE: u32 = 1 << 1;
const WRITE_READY: u32 = 1 << 4;
const READ_READY: u32 = 1 << 5;
const ANY_ERROR: u32 = 1 << 15;
const CLEAR_ALL: u32 = 0xFFFF_FFFF;
const TIMEOUT_MICROSECONDS: u32 = 500_000;

/// Why a read failed, with the controller's interrupt status where it has one.
pub enum SdError {
    /// The buffer's length isn't a whole number of blocks, or is empty.
    BadLength,
    /// The controller reported an error, and may need a reset before it works again.
    Failed(u32),
    TimedOut(u32),
}

impl fmt::Display for SdError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::BadLength => write!(formatter, "the buffer isn't a whole number of blocks"),
            Self::Failed(status) => write!(formatter, "the controller reported an error, status {status:#010x}"),
            Self::TimedOut(status) => write!(formatter, "timed out, status {status:#010x}"),
        }
    }
}

fn read_register(offset: usize) -> u32 {
    // SAFETY: a register of the SD controller on this board, which the firmware left
    // clocked. The MMU maps peripherals as device memory.
    unsafe { read_volatile((EMMC2 + offset) as *const u32) }
}

fn write_register(offset: usize, value: u32) {
    // SAFETY: as in `read_register`.
    unsafe { write_volatile((EMMC2 + offset) as *mut u32, value) };
}

/// Waits for an interrupt status flag and clears it.
fn wait_for(flag: u32) -> Result<(), SdError> {
    let start = microseconds();
    loop {
        let status = read_register(INTERRUPT_STATUS);
        if status & ANY_ERROR != 0 {
            return Err(SdError::Failed(status));
        }
        if status & flag != 0 {
            write_register(INTERRUPT_STATUS, flag);
            return Ok(());
        }
        if microseconds().wrapping_sub(start) > TIMEOUT_MICROSECONDS {
            return Err(SdError::TimedOut(status));
        }
    }
}

/// Writes one block to the card, and waits until the card has taken it.
///
/// Nothing here stops a write to the wrong place, so only `card.rs` calls this.
pub fn write_block(block: u32, data: &[u8; BLOCK_BYTES]) -> Result<(), SdError> {
    write_register(INTERRUPT_STATUS, CLEAR_ALL);
    write_register(BLOCK_COUNT_SIZE, 1 << BLOCK_COUNT_SHIFT | BLOCK_BYTES as u32);
    write_register(ARGUMENT, block);
    write_register(COMMAND, WRITE_ONE);
    wait_for(COMMAND_COMPLETE)?;
    wait_for(WRITE_READY)?;
    for word in data.chunks_exact(WORD_BYTES) {
        write_register(DATA_PORT, u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
    }
    wait_for(TRANSFER_COMPLETE)
}

/// Fills `buffer` from the card, starting at block number `first`. Its length must be
/// a whole number of blocks.
pub fn read_blocks(first: u32, buffer: &mut [u8]) -> Result<(), SdError> {
    if buffer.is_empty() || !buffer.len().is_multiple_of(BLOCK_BYTES) {
        return Err(SdError::BadLength);
    }
    let count = (buffer.len() / BLOCK_BYTES) as u32;

    write_register(INTERRUPT_STATUS, CLEAR_ALL);
    write_register(BLOCK_COUNT_SIZE, count << BLOCK_COUNT_SHIFT | BLOCK_BYTES as u32);
    write_register(ARGUMENT, first);
    write_register(COMMAND, if count == 1 { READ_ONE } else { READ_MANY });
    wait_for(COMMAND_COMPLETE)?;

    for block in buffer.chunks_exact_mut(BLOCK_BYTES) {
        wait_for(READ_READY)?;
        for word in block.chunks_exact_mut(WORD_BYTES) {
            word.copy_from_slice(&read_register(DATA_PORT).to_le_bytes());
        }
    }
    wait_for(TRANSFER_COMPLETE)
}
