//! Finds a file on a FAT32 filesystem, and overwrites its data in place.
//!
//! That's all this can change. It can't create, grow, rename or delete anything, and has no
//! code that writes to the FAT or to a directory, so it can't damage the filesystem's
//! structure, or any other file. Writes go through `write_data_block`, the only place that
//! calls `Blocks::write`, which refuses anything before the data area.
//!
//! Established libraries such as FatFs do far more, and more than should be near a boot
//! card. See `tests/real_image.rs` for how this is checked against Linux's own driver.
#![cfg_attr(not(test), no_std)]

pub const BLOCK_BYTES: usize = 512;
pub type Block = [u8; BLOCK_BYTES];

/// The device couldn't do what was asked. It reports why itself, if it can.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceError;

/// Whatever holds the filesystem, addressed in 512-byte blocks from its start.
pub trait Blocks {
    /// # Errors
    /// If the device can't read that block.
    fn read(&mut self, block: u32, data: &mut Block) -> Result<(), DeviceError>;
    /// # Errors
    /// If the device can't write that block.
    fn write(&mut self, block: u32, data: &Block) -> Result<(), DeviceError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FatError {
    /// The device reported an error.
    Device,
    /// This isn't a FAT32 filesystem this understands.
    NotFat32,
    NotFound,
    /// The file's chain of clusters isn't sound, or doesn't match its size.
    BadChain,
    /// The data doesn't fit the file's allocated space.
    TooLarge,
    /// What was read back isn't what was written.
    Mismatch,
}

const SIGNATURE_AT: usize = 510;
const SIGNATURE: [u8; 2] = [0x55, 0xAA];
const BYTES_PER_SECTOR_AT: usize = 11;
const SECTORS_PER_CLUSTER_AT: usize = 13;
const RESERVED_SECTORS_AT: usize = 14;
const FAT_COUNT_AT: usize = 16;
const ROOT_ENTRIES_AT: usize = 17;
const TOTAL_SECTORS_16_AT: usize = 19;
const FAT_SIZE_16_AT: usize = 22;
const TOTAL_SECTORS_32_AT: usize = 32;
const FAT_SIZE_32_AT: usize = 36;
const ROOT_CLUSTER_AT: usize = 44;

const FIRST_CLUSTER: u32 = 2;
const FAT_ENTRY_BYTES: u32 = 4;
const FAT_ENTRY_MASK: u32 = 0x0FFF_FFFF;
const FAT_END_OF_CHAIN: u32 = 0x0FFF_FFF8;

const ENTRY_BYTES: usize = 32;
const ENTRY_END: u8 = 0x00;
const ENTRY_ATTRIBUTES_AT: usize = 11;
const ENTRY_CLUSTER_HIGH_AT: usize = 20;
const ENTRY_CLUSTER_LOW_AT: usize = 26;
const ENTRY_SIZE_AT: usize = 28;
const SHORT_NAME_BYTES: usize = 11;
const SHORT_STEM_BYTES: usize = 8;

const ATTRIBUTE_VOLUME_ID: u8 = 0x08;
const ATTRIBUTE_DIRECTORY: u8 = 0x10;
const ATTRIBUTE_LONG_NAME: u8 = 0x0F;
const LONG_ORDER_MASK: u8 = 0x1F;
const LONG_CHECKSUM_AT: usize = 13;
const LONG_CHARS_PER_ENTRY: usize = 13;
/// Where each of a long name entry's 13 characters is, as UTF-16.
const LONG_CHAR_OFFSETS: [usize; LONG_CHARS_PER_ENTRY] = [1, 3, 5, 7, 9, 14, 16, 18, 20, 22, 24, 28, 30];
/// The longest name this will match, which is plenty for files this project names.
const MAX_NAME_BYTES: usize = 64;

fn u16_at(data: &[u8], at: usize) -> u32 {
    u32::from(u16::from_le_bytes([data[at], data[at + 1]]))
}

fn u32_at(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

/// A filesystem's layout.
#[derive(Debug)]
pub struct Volume {
    fat_start: u32,
    fat_blocks: u32,
    data_start: u32,
    blocks_per_cluster: u32,
    /// One past the highest valid cluster number.
    cluster_limit: u32,
    root_cluster: u32,
}

/// A file's first cluster and size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct File {
    pub first_cluster: u32,
    pub size: u32,
}

impl Volume {
    /// Reads the layout from the filesystem's first block, starting from nothing assumed.
    ///
    /// # Errors
    /// If it can't be read, or isn't FAT32 with 512-byte sectors.
    pub fn open<D: Blocks>(device: &mut D) -> Result<Self, FatError> {
        let mut boot = [0; BLOCK_BYTES];
        device.read(0, &mut boot).map_err(|DeviceError| FatError::Device)?;
        let blocks_per_cluster = u32::from(boot[SECTORS_PER_CLUSTER_AT]);
        let reserved = u16_at(&boot, RESERVED_SECTORS_AT);
        let fat_count = u32::from(boot[FAT_COUNT_AT]);
        let fat_blocks = u32_at(&boot, FAT_SIZE_32_AT);
        let total = u32_at(&boot, TOTAL_SECTORS_32_AT);
        let is_fat32 = boot[SIGNATURE_AT..SIGNATURE_AT + SIGNATURE.len()] == SIGNATURE
            && u16_at(&boot, BYTES_PER_SECTOR_AT) as usize == BLOCK_BYTES
            && blocks_per_cluster.is_power_of_two()
            && reserved != 0
            && fat_count != 0
            && u16_at(&boot, ROOT_ENTRIES_AT) == 0
            && u16_at(&boot, TOTAL_SECTORS_16_AT) == 0
            && u16_at(&boot, FAT_SIZE_16_AT) == 0
            && fat_blocks != 0
            && total != 0;
        if !is_fat32 {
            return Err(FatError::NotFat32);
        }
        let data_start = fat_blocks.checked_mul(fat_count).and_then(|fats| fats.checked_add(reserved)).ok_or(FatError::NotFat32)?;
        let clusters = total.checked_sub(data_start).ok_or(FatError::NotFat32)? / blocks_per_cluster;
        let cluster_limit = clusters.checked_add(FIRST_CLUSTER).ok_or(FatError::NotFat32)?;
        let volume = Self { fat_start: reserved, fat_blocks, data_start, blocks_per_cluster, cluster_limit, root_cluster: u32_at(&boot, ROOT_CLUSTER_AT) };
        if !volume.is_valid_cluster(volume.root_cluster) {
            return Err(FatError::NotFat32);
        }
        Ok(volume)
    }

    const fn is_valid_cluster(&self, cluster: u32) -> bool {
        cluster >= FIRST_CLUSTER && cluster < self.cluster_limit
    }

    /// Returns the next cluster in a chain, or `None` at its end.
    fn next_cluster<D: Blocks>(&self, device: &mut D, cluster: u32) -> Result<Option<u32>, FatError> {
        if !self.is_valid_cluster(cluster) {
            return Err(FatError::BadChain);
        }
        let offset = cluster.checked_mul(FAT_ENTRY_BYTES).ok_or(FatError::BadChain)?;
        let block = offset / BLOCK_BYTES as u32;
        if block >= self.fat_blocks {
            return Err(FatError::BadChain);
        }
        let mut data = [0; BLOCK_BYTES];
        device.read(self.fat_start + block, &mut data).map_err(|DeviceError| FatError::Device)?;
        let next = u32_at(&data, (offset % BLOCK_BYTES as u32) as usize) & FAT_ENTRY_MASK;
        if next >= FAT_END_OF_CHAIN {
            return Ok(None);
        }
        // Free clusters and bad-cluster marks both land outside the valid range. `find` reads
        // from whatever this returns, so it has to be checked here.
        if !self.is_valid_cluster(next) {
            return Err(FatError::BadChain);
        }
        Ok(Some(next))
    }

    fn cluster_block(&self, cluster: u32, index: u32) -> u32 {
        self.data_start + (cluster - FIRST_CLUSTER) * self.blocks_per_cluster + index
    }

    /// Finds a file in the root directory by its name, ignoring ASCII case.
    ///
    /// # Errors
    /// If the directory can't be read, or has no such file.
    pub fn find<D: Blocks>(&self, device: &mut D, name: &str) -> Result<File, FatError> {
        let mut long_name = LongName::new();
        let mut cluster = self.root_cluster;
        let mut clusters_left = self.cluster_limit;
        let mut data = [0; BLOCK_BYTES];
        loop {
            for index in 0..self.blocks_per_cluster {
                device.read(self.cluster_block(cluster, index), &mut data).map_err(|DeviceError| FatError::Device)?;
                for entry in data.chunks_exact(ENTRY_BYTES) {
                    match entry[0] {
                        ENTRY_END => return Err(FatError::NotFound),
                        _ if entry[ENTRY_ATTRIBUTES_AT] == ATTRIBUTE_LONG_NAME => long_name.add(entry),
                        _ => {
                            let skip = entry[ENTRY_ATTRIBUTES_AT] & (ATTRIBUTE_VOLUME_ID | ATTRIBUTE_DIRECTORY) != 0;
                            if !skip && (long_name.matches(name, entry) || short_name_matches(name, entry)) {
                                let high = u16_at(entry, ENTRY_CLUSTER_HIGH_AT);
                                let low = u16_at(entry, ENTRY_CLUSTER_LOW_AT);
                                return Ok(File { first_cluster: high << u16::BITS | low, size: u32_at(entry, ENTRY_SIZE_AT) });
                            }
                            long_name.clear();
                        }
                    }
                }
            }
            clusters_left = clusters_left.checked_sub(1).ok_or(FatError::BadChain)?;
            cluster = self.next_cluster(device, cluster)?.ok_or(FatError::NotFound)?;
        }
    }

    /// Walks a file's chain, returning its allocated bytes, if it's sound and exactly the
    /// length its size calls for, so nothing written can run off the end of it.
    fn check_chain<D: Blocks>(&self, device: &mut D, file: &File) -> Result<u64, FatError> {
        let cluster_bytes = u64::from(self.blocks_per_cluster) * BLOCK_BYTES as u64;
        let needed = u64::from(file.size).div_ceil(cluster_bytes);
        let mut cluster = file.first_cluster;
        for _ in 1..needed {
            cluster = self.next_cluster(device, cluster)?.ok_or(FatError::BadChain)?;
        }
        if self.next_cluster(device, cluster)?.is_some() {
            return Err(FatError::BadChain);
        }
        Ok(needed * cluster_bytes)
    }

    /// The only place anything gets written.
    fn write_data_block<D: Blocks>(&self, device: &mut D, block: u32, data: &Block) -> Result<(), FatError> {
        assert!(block >= self.data_start, "a write would have reached the FAT");
        device.write(block, data).map_err(|DeviceError| FatError::Device)
    }

    /// Replaces a file's contents, without changing its size or anything else: the rest
    /// of it is filled with zeros. Then reads the whole file back to check it. A failure
    /// partway leaves the file partly written, but nothing else on the filesystem changed.
    ///
    /// # Errors
    /// Before writing, if the data doesn't fit or the file's chain isn't sound. Afterward,
    /// if the device fails or what's read back differs.
    pub fn overwrite<D: Blocks>(&self, device: &mut D, file: &File, contents: &[u8]) -> Result<(), FatError> {
        if contents.len() as u64 > u64::from(file.size) {
            return Err(FatError::TooLarge);
        }
        self.check_chain(device, file)?;
        self.for_each_block(device, file, |device, volume, block, data_index| {
            let data = block_of(contents, data_index);
            volume.write_data_block(device, block, &data)
        })?;
        self.for_each_block(device, file, |device, _, block, data_index| {
            let mut read_back = [0; BLOCK_BYTES];
            device.read(block, &mut read_back).map_err(|DeviceError| FatError::Device)?;
            if read_back == block_of(contents, data_index) {
                Ok(())
            } else {
                Err(FatError::Mismatch)
            }
        })
    }

    /// Calls `action` for each block a file occupies, with its position in the file.
    fn for_each_block<D: Blocks>(
        &self,
        device: &mut D,
        file: &File,
        mut action: impl FnMut(&mut D, &Self, u32, usize) -> Result<(), FatError>,
    ) -> Result<(), FatError> {
        let blocks = (file.size as usize).div_ceil(BLOCK_BYTES);
        let mut cluster = file.first_cluster;
        let mut done = 0;
        while done < blocks {
            for index in 0..self.blocks_per_cluster {
                if done == blocks {
                    break;
                }
                action(device, self, self.cluster_block(cluster, index), done)?;
                done += 1;
            }
            if done < blocks {
                cluster = self.next_cluster(device, cluster)?.ok_or(FatError::BadChain)?;
            }
        }
        Ok(())
    }
}

/// One block of a file's new contents, with zeros past the end of them.
fn block_of(contents: &[u8], index: usize) -> Block {
    let mut block = [0; BLOCK_BYTES];
    let start = (index * BLOCK_BYTES).min(contents.len());
    let end = (start + BLOCK_BYTES).min(contents.len());
    block[..end - start].copy_from_slice(&contents[start..end]);
    block
}

/// The long name built up from the entries before a file's own.
struct LongName {
    characters: [u16; MAX_NAME_BYTES],
    length: usize,
    checksum: u8,
}

impl LongName {
    const fn new() -> Self {
        Self { characters: [0; MAX_NAME_BYTES], length: 0, checksum: 0 }
    }

    const fn clear(&mut self) {
        self.length = 0;
    }

    fn add(&mut self, entry: &[u8]) {
        self.checksum = entry[LONG_CHECKSUM_AT];
        let position = usize::from(entry[0] & LONG_ORDER_MASK).saturating_sub(1) * LONG_CHARS_PER_ENTRY;
        for (index, offset) in LONG_CHAR_OFFSETS.iter().enumerate() {
            let character = u16::from_le_bytes([entry[*offset], entry[*offset + 1]]);
            if character == 0 || character == u16::MAX {
                break;
            }
            let at = position + index;
            if at < MAX_NAME_BYTES {
                self.characters[at] = character;
                self.length = self.length.max(at + 1);
            }
        }
    }

    /// Whether this is the given name, and belongs to the file in `entry`, by its checksum.
    /// A long name left behind by something that doesn't understand them fails that.
    fn matches(&self, name: &str, entry: &[u8]) -> bool {
        let checksum = entry[..SHORT_NAME_BYTES].iter().fold(0u8, |sum, byte| sum.rotate_right(1).wrapping_add(*byte));
        self.checksum == checksum
            && self.length == name.len()
            && name.bytes().zip(&self.characters).all(|(wanted, have)| u8::try_from(*have).is_ok_and(|have| have.eq_ignore_ascii_case(&wanted)))
    }
}

fn short_name_matches(name: &str, entry: &[u8]) -> bool {
    let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
    if stem.len() > SHORT_STEM_BYTES || extension.len() > SHORT_NAME_BYTES - SHORT_STEM_BYTES {
        return false;
    }
    let mut wanted = [b' '; SHORT_NAME_BYTES];
    wanted[..stem.len()].copy_from_slice(stem.as_bytes());
    wanted[SHORT_STEM_BYTES..SHORT_STEM_BYTES + extension.len()].copy_from_slice(extension.as_bytes());
    wanted.eq_ignore_ascii_case(&entry[..SHORT_NAME_BYTES])
}
