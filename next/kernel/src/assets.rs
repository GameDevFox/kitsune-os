//! Images in their own region of RAM, in the format `scripts/make-assets.py` writes.
//!
//! The same bytes can sit in raw blocks on the SD card, after its partition, and then
//! each image is read into its place in the region the first time something needs it.
//! Otherwise the region must already hold them, as it does in QEMU, or with an image
//! file that includes them. The card comes first because a watchdog reboot doesn't
//! clear RAM, so old images can look loaded.

use core::slice;

use crate::board::{self, read_blocks, BLOCK_BYTES};

/// Slot numbers, matching the order in `scripts/make-assets.py`.
pub const LOGO: usize = 0;
pub const AKI: usize = 1;
pub const AKI_MOUTH: usize = 6;
pub const AKI_X2: usize = 7;
pub const AKI_X2_MOUTH: usize = 8;
pub const AKI_X4: usize = 9;
pub const AKI_X4_MOUTH: usize = 10;
pub const AKI_ORIGINAL: usize = 11;
pub const AKI_ORIGINAL_MOUTH: usize = 12;
/// A sound clip rather than an image, in `sound.rs`'s form, and empty on a machine that
/// names no recording for it.
pub const VOICE: usize = 13;

const MAGIC: &[u8] = b"KITSASS2";
const COUNT_AT: usize = 8;
const TABLE_AT: usize = 12;
const ENTRY_BYTES: usize = 8;
const WORD_BYTES: usize = 4;
const MAX_SLOTS: usize = 32;
const MICROSECONDS_PER_MILLISECOND: u32 = 1000;
const KILOBYTE: usize = 1024;

fn read_u32(data: &[u8], at: usize) -> Option<usize> {
    let bytes = data.get(at..at + WORD_BYTES)?;
    let value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    Some(value as usize)
}

/// Whether `data` starts with a table of images, entirely within it.
fn has_table(data: &[u8]) -> bool {
    data.get(..MAGIC.len()) == Some(MAGIC)
        && read_u32(data, COUNT_AT).is_some_and(|count| count <= MAX_SLOTS && TABLE_AT + count * ENTRY_BYTES <= data.len())
}

/// Reads the table at the start of `region`, and images into the rest of it.
pub struct Assets {
    region: &'static mut [u8],
    /// The block of the card holding the region's first byte, if images come from there.
    card_block: Option<u32>,
    loaded: [bool; MAX_SLOTS],
}

impl Assets {
    /// Finds the images, from raw blocks on the card starting at `card_block` if it has
    /// a table there, or else in `region`. Returns `None` if neither has them.
    pub fn find(region: &'static mut [u8], card_block: Option<u32>) -> Option<Self> {
        if let Some(block) = card_block {
            let mut first = [0; BLOCK_BYTES];
            if read_blocks(block, &mut first).is_ok() && has_table(&first) {
                region[..BLOCK_BYTES].copy_from_slice(&first);
                return Some(Self { region, card_block, loaded: [false; MAX_SLOTS] });
            }
        }
        has_table(region).then_some(Self { region, card_block: None, loaded: [true; MAX_SLOTS] })
    }

    /// Whether images are coming from the card, rather than already being in memory.
    pub fn on_card(&self) -> bool {
        self.card_block.is_some()
    }

    /// The bytes of one image, reading it from the card if this is the first time. Returns
    /// `None` if the table doesn't describe it properly, or it can't be read.
    pub fn get(&mut self, slot: usize) -> Option<&'static [u8]> {
        if slot >= read_u32(self.region, COUNT_AT)? || slot >= MAX_SLOTS {
            return None;
        }
        let entry = TABLE_AT + slot * ENTRY_BYTES;
        let offset = read_u32(self.region, entry)?;
        let length = read_u32(self.region, entry + WORD_BYTES)?;
        let end = offset.checked_add(length)?;
        self.region.get(offset..end)?;

        if !self.loaded[slot] {
            self.load(offset, length)?;
            self.loaded[slot] = true;
        }
        // SAFETY: checked above to be inside the region, which lasts forever. A slot is
        // only written the first time it's asked for, before this is handed out.
        Some(unsafe { slice::from_raw_parts(self.region.as_ptr().add(offset), length) })
    }

    fn load(&mut self, offset: usize, length: usize) -> Option<()> {
        let rounded = length.next_multiple_of(BLOCK_BYTES);
        if !offset.is_multiple_of(BLOCK_BYTES) {
            return None;
        }
        let block = self.card_block? + (offset / BLOCK_BYTES) as u32;
        let target = self.region.get_mut(offset..offset + rounded)?;
        let start = board::microseconds();
        if let Err(error) = read_blocks(block, target) {
            println!("Couldn't read an image: {error}");
            return None;
        }
        let elapsed = board::microseconds().wrapping_sub(start) / MICROSECONDS_PER_MILLISECOND;
        println!("Read {} KB from the SD card in {elapsed} ms.", length / KILOBYTE);
        Some(())
    }
}
