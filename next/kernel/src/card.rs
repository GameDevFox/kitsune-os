//! What's on the SD card, read through whatever the board offers.

use fat::{Block, Blocks, DeviceError, FatError, Volume};

use crate::board::{read_blocks, write_block, BLOCK_BYTES};

const PARTITION_START_AT: usize = 446 + 8;
const PARTITION_SIZE_AT: usize = 446 + 12;
const SIGNATURE_AT: usize = 510;
const SIGNATURE: [u8; 2] = [0x55, 0xAA];
const FAT32_VOLUME_ID_AT: usize = 0x43;
const WORD_BYTES: usize = 4;

/// Where images sit on the card: 5 GiB in, which a survey found blank. The space after
/// the first partition isn't, as it holds old data from the card's previous use.
/// See "What else is on the card" in `docs/hardware-notes.md`.
const IMAGE_AREA_BLOCK: u32 = 10 * 1024 * 1024;

fn word_at(block: &[u8; BLOCK_BYTES], at: usize) -> u32 {
    let mut bytes = [0; WORD_BYTES];
    bytes.copy_from_slice(&block[at..at + WORD_BYTES]);
    u32::from_le_bytes(bytes)
}

/// Where images start on the card. Returns `None` if the card can't be read, has no
/// partition table, or its first partition reaches that far.
pub fn image_area() -> Option<u32> {
    let mut block = [0; BLOCK_BYTES];
    read_blocks(0, &mut block).ok()?;
    if block[SIGNATURE_AT..SIGNATURE_AT + SIGNATURE.len()] != SIGNATURE {
        return None;
    }
    let partition_end = word_at(&block, PARTITION_START_AT).checked_add(word_at(&block, PARTITION_SIZE_AT))?;
    (partition_end <= IMAGE_AREA_BLOCK).then_some(IMAGE_AREA_BLOCK)
}

/// The kernels the firmware can boot, which are worth reporting.
const KERNEL_FILES: [&str; 2] = ["kitsune-next.img", "kitsune.img"];

/// The only files the kernel may ever change. The old kernel isn't one of them.
const WRITABLE_FILES: [&str; 3] = ["kitsune-next.img", "kitsune-try.img", "update-test.bin"];

/// The first partition, addressed from its own start, which keeps everything inside it.
struct Partition {
    first_block: u32,
    /// How many blocks it has.
    blocks: u32,
}

impl Blocks for Partition {
    fn read(&mut self, block: u32, data: &mut Block) -> Result<(), DeviceError> {
        read_blocks(self.first_block + block, data).map_err(|_| DeviceError)
    }

    fn write(&mut self, block: u32, data: &Block) -> Result<(), DeviceError> {
        if block >= self.blocks {
            return Err(DeviceError);
        }
        write_block(self.first_block + block, data).map_err(|_| DeviceError)
    }
}

/// Prints where the kernels are, through the filesystem, which shows that it's understood.
fn report_files(first_block: u32) {
    let mut partition = Partition { first_block, blocks: 0 };
    let volume = match Volume::open(&mut partition) {
        Ok(volume) => volume,
        Err(error) => {
            println!("SD card: couldn't read the filesystem: {error:?}");
            return;
        }
    };
    for name in KERNEL_FILES {
        match volume.find(&mut partition, name) {
            Ok(file) => println!("SD card: {name} is {} bytes, from cluster {}.", file.size, file.first_cluster),
            Err(error) => println!("SD card: {name}: {error:?}"),
        }
    }
}

/// Why a file couldn't be overwritten.
#[derive(Debug)]
pub enum OverwriteError {
    /// It's not a file the kernel may change, so nothing was touched.
    NotAllowed,
    /// The card's partition table couldn't be read, so nothing was touched.
    NoPartition,
    // Only ever printed, which the compiler doesn't count as reading it.
    #[allow(dead_code)]
    Fat(FatError),
}

/// Replaces a file's contents in place, with zeros filling the rest of its size, and reads
/// it back to check. The file must be one of `WRITABLE_FILES`.
pub fn overwrite(name: &str, contents: &[u8]) -> Result<(), OverwriteError> {
    if !WRITABLE_FILES.contains(&name) {
        return Err(OverwriteError::NotAllowed);
    }
    let mut block = [0; BLOCK_BYTES];
    read_blocks(0, &mut block).map_err(|_| OverwriteError::NoPartition)?;
    if block[SIGNATURE_AT..SIGNATURE_AT + SIGNATURE.len()] != SIGNATURE {
        return Err(OverwriteError::NoPartition);
    }
    let mut partition = Partition { first_block: word_at(&block, PARTITION_START_AT), blocks: word_at(&block, PARTITION_SIZE_AT) };
    let volume = Volume::open(&mut partition).map_err(OverwriteError::Fat)?;
    let file = volume.find(&mut partition, name).map_err(OverwriteError::Fat)?;
    volume.overwrite(&mut partition, &file, contents).map_err(OverwriteError::Fat)
}

/// Prints the first partition's FAT32 volume ID, which shows that reading works.
pub fn report() {
    let mut block = [0; BLOCK_BYTES];
    if let Err(error) = read_blocks(0, &mut block) {
        println!("Couldn't read the SD card: {error}");
        return;
    }
    if block[SIGNATURE_AT..SIGNATURE_AT + SIGNATURE.len()] != SIGNATURE {
        println!("The SD card has no partition table.");
        return;
    }
    let partition = word_at(&block, PARTITION_START_AT);
    if let Err(error) = read_blocks(partition, &mut block) {
        println!("Couldn't read block {partition} of the SD card: {error}");
        return;
    }
    let id = word_at(&block, FAT32_VOLUME_ID_AT);
    println!("SD card: partition at block {partition}, volume ID {:04X}-{:04X}.", id >> 16, id & 0xFFFF);
    report_files(partition);
}
