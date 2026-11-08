//! Reads a flattened device tree, as the firmware passes it, without copying anything:
//! each lookup walks the tree from the start. The format is in the Devicetree
//! Specification, chapter 5. Everything in it is big-endian.
//!
//! `walk` hands out the tree one event at a time, which both printing it and finding
//! something in it are built on. libfdt walks a tree the same way, token by token.
#![cfg_attr(not(test), no_std)]

mod dts;

pub use dts::write as write_dts;

const MAGIC: u32 = 0xD00D_FEED;
const MAGIC_AT: usize = 0;
const TOTAL_SIZE_AT: usize = 4;
const STRUCTURE_AT: usize = 8;
const STRINGS_AT: usize = 12;
const VERSION_AT: usize = 20;
const STRINGS_SIZE_AT: usize = 32;
const STRUCTURE_SIZE_AT: usize = 36;
pub const HEADER_BYTES: usize = 40;
/// Versions before this lack the sizes above, and a tree's header gives it as its
/// lowest compatible version, so anything older is refused.
const MIN_VERSION: u32 = 17;

const WORD_BYTES: usize = 4;
const CELL_BYTES: usize = 4;
const BEGIN_NODE: u32 = 1;
const END_NODE: u32 = 2;
const PROPERTY: u32 = 3;
const NOP: u32 = 4;
const END: u32 = 9;

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let word = bytes.get(at..at.checked_add(WORD_BYTES)?)?;
    Some(u32::from_be_bytes([word[0], word[1], word[2], word[3]]))
}

/// The text before the first zero byte from `at`, if it's there and is UTF-8.
fn string_at(bytes: &[u8], at: usize) -> Option<&str> {
    let rest = bytes.get(at..)?;
    let length = rest.iter().position(|&byte| byte == 0)?;
    core::str::from_utf8(&rest[..length]).ok()
}

/// The total size a tree's header claims, so its caller knows how much to hand to `new`.
/// `None` if the bytes don't start with a device tree's header.
pub fn total_size(header: &[u8]) -> Option<usize> {
    if read_u32(header, MAGIC_AT)? != MAGIC || header.len() < HEADER_BYTES {
        return None;
    }
    Some(read_u32(header, TOTAL_SIZE_AT)? as usize)
}

#[derive(Clone, Copy)]
pub struct DeviceTree<'a> {
    bytes: &'a [u8],
    structure: &'a [u8],
    strings: &'a [u8],
    size: usize,
    version: u32,
}

impl<'a> DeviceTree<'a> {
    /// Checks the header, and that its blocks lie within `bytes`.
    pub fn new(bytes: &'a [u8]) -> Option<Self> {
        let size = total_size(bytes)?;
        let version = read_u32(bytes, VERSION_AT)?;
        if version < MIN_VERSION || size > bytes.len() {
            return None;
        }
        let block = |at, size_at| {
            let start = read_u32(bytes, at)? as usize;
            let length = read_u32(bytes, size_at)? as usize;
            bytes.get(start..start.checked_add(length)?)
        };
        let structure = block(STRUCTURE_AT, STRUCTURE_SIZE_AT)?;
        let strings = block(STRINGS_AT, STRINGS_SIZE_AT)?;
        Some(Self { bytes, structure, strings, size, version })
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    /// Every node and property, in order. The walk just ends if the tree turns out to
    /// be malformed; `Walk::broken` says whether it did.
    pub fn walk(&self) -> Walk<'a> {
        Walk { tree: *self, at: 0, depth: 0, finished: false, broken: false }
    }

    /// The value of a property of the node at `path`, such as `/memory` and `reg`. A
    /// path's part without an `@` matches a node with any unit address, so `/memory`
    /// finds `memory@0`.
    pub fn property(&self, path: &str, name: &str) -> Option<&'a [u8]> {
        let wanted: &str = path.trim_start_matches('/');
        let parts = || wanted.split('/').filter(|part| !part.is_empty());
        let target = parts().count();
        // How many of the path's parts the nodes we're inside have matched.
        let mut matched = 0;
        for event in self.walk() {
            match event {
                Event::Node { name: node, depth } => {
                    if depth > 0 && depth == matched + 1 && parts().nth(matched).is_some_and(|part| names_match(node, part)) {
                        matched += 1;
                    }
                }
                Event::EndNode { depth } => {
                    if depth > 0 && depth == matched {
                        matched -= 1;
                    }
                }
                Event::Property { name: property, value, depth } => {
                    if depth == target && matched == target && property == name {
                        return Some(value);
                    }
                }
            }
        }
        None
    }

    /// The path an alias in `/aliases` stands for, such as `serial0`.
    pub fn alias(&self, name: &str) -> Option<&'a str> {
        self.path_named("/aliases", name)
    }

    /// The path a label in `/__symbols__` stands for, such as `gicv2`. These are the
    /// labels in the tree's source, which `dtc` keeps when asked, as for the Pi.
    pub fn symbol(&self, name: &str) -> Option<&'a str> {
        self.path_named("/__symbols__", name)
    }

    fn path_named(&self, table: &str, name: &str) -> Option<&'a str> {
        let value = self.property(table, name)?;
        core::str::from_utf8(value.strip_suffix(&[0])?).ok()
    }

    /// A whole-number property, such as `#address-cells`, which is one cell.
    pub fn cell(&self, path: &str, name: &str) -> Option<u32> {
        let value = self.property(path, name)?;
        (value.len() == CELL_BYTES).then(|| read_u32(value, 0))?
    }
}

fn names_match(node: &str, part: &str) -> bool {
    node == part || (!part.contains('@') && node.split('@').next() == Some(part))
}

/// One step through the tree. Depth 0 is the root node, whose name is empty, and a
/// property's depth is its node's.
#[derive(Debug, PartialEq, Eq)]
pub enum Event<'a> {
    Node { name: &'a str, depth: usize },
    Property { name: &'a str, value: &'a [u8], depth: usize },
    EndNode { depth: usize },
}

pub struct Walk<'a> {
    tree: DeviceTree<'a>,
    at: usize,
    /// How many nodes we're inside.
    depth: usize,
    finished: bool,
    broken: bool,
}

impl Walk<'_> {
    /// Whether the walk ended early, at something that made no sense.
    pub fn broken(&self) -> bool {
        self.broken
    }
}

impl<'a> Walk<'a> {
    fn fail(&mut self) -> Option<Event<'a>> {
        self.finished = true;
        self.broken = true;
        None
    }

    fn word(&mut self) -> Option<u32> {
        let word = read_u32(self.tree.structure, self.at)?;
        self.at += WORD_BYTES;
        Some(word)
    }

    /// Moves past `length` bytes, then to the next word.
    fn skip(&mut self, length: usize) {
        self.at = (self.at + length).next_multiple_of(WORD_BYTES);
    }
}

impl<'a> Iterator for Walk<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Event<'a>> {
        if self.finished {
            return None;
        }
        loop {
            let Some(token) = self.word() else { return self.fail() };
            match token {
                BEGIN_NODE => {
                    let Some(name) = string_at(self.tree.structure, self.at) else { return self.fail() };
                    self.skip(name.len() + 1);
                    let depth = self.depth;
                    self.depth += 1;
                    return Some(Event::Node { name, depth });
                }
                END_NODE => {
                    let Some(depth) = self.depth.checked_sub(1) else { return self.fail() };
                    self.depth = depth;
                    return Some(Event::EndNode { depth });
                }
                PROPERTY => {
                    let (Some(length), Some(name_at)) = (self.word(), self.word()) else { return self.fail() };
                    let length = length as usize;
                    let value = self.tree.structure.get(self.at..self.at + length);
                    let name = string_at(self.tree.strings, name_at as usize);
                    let (Some(value), Some(name), Some(depth)) = (value, name, self.depth.checked_sub(1)) else {
                        return self.fail();
                    };
                    self.skip(length);
                    return Some(Event::Property { name, value, depth });
                }
                NOP => {}
                END => {
                    self.finished = true;
                    if self.depth != 0 {
                        self.broken = true;
                    }
                    return None;
                }
                _ => return self.fail(),
            }
        }
    }
}

/// Splits a value into 32-bit cells, ignoring any bytes left over.
pub fn cells(value: &[u8]) -> impl Iterator<Item = u32> + '_ {
    value.chunks_exact(CELL_BYTES).map(|cell| u32::from_be_bytes([cell[0], cell[1], cell[2], cell[3]]))
}

/// Reads a number made of `count` cells from the front of `cells`, most significant
/// first. `None` if there are too few, or it's wider than 64 bits.
fn number(cells: &mut impl Iterator<Item = u32>, count: u32) -> Option<u64> {
    const CELL_BITS: u32 = 32;
    const MOST_CELLS: u32 = 2;
    if count > MOST_CELLS {
        return None;
    }
    (0..count).try_fold(0u64, |value, _| Some(value << CELL_BITS | u64::from(cells.next()?)))
}

/// The address and size pairs in a `reg` value, given the parent's `#address-cells`
/// and `#size-cells`.
pub fn regions(value: &[u8], address_cells: u32, size_cells: u32) -> impl Iterator<Item = (u64, u64)> + '_ {
    let mut cells = cells(value);
    core::iter::from_fn(move || Some((number(&mut cells, address_cells)?, number(&mut cells, size_cells)?)))
}

/// Translates an address on a bus to its parent's, through the bus's `ranges` value,
/// given the bus's `#address-cells` and `#size-cells` and its parent's
/// `#address-cells`. `None` if no range covers it. An empty `ranges` means the same
/// addresses on both sides.
pub fn translate(address: u64, ranges: &[u8], child_cells: u32, parent_cells: u32, size_cells: u32) -> Option<u64> {
    if ranges.is_empty() {
        return Some(address);
    }
    let mut cells = cells(ranges);
    while let (Some(child), Some(parent), Some(size)) =
        (number(&mut cells, child_cells), number(&mut cells, parent_cells), number(&mut cells, size_cells))
    {
        if address >= child && address - child < size {
            return Some(parent + (address - child));
        }
    }
    None
}

/// How a property's value is best shown, for printing a tree.
#[derive(Debug, PartialEq, Eq)]
pub enum Shown<'a> {
    Empty,
    /// One or more strings, each ending with a zero byte, as `compatible` holds.
    Strings(&'a [u8]),
    Cells(&'a [u8]),
    Bytes(&'a [u8]),
}

/// Control characters `dtc` writes as escapes, such as `\t`, so they can be in strings.
pub(crate) const ESCAPED: &[(u8, char)] =
    &[(0x07, 'a'), (0x08, 'b'), (b'\t', 't'), (b'\n', 'n'), (0x0B, 'v'), (0x0C, 'f'), (b'\r', 'r')];

/// Decides as `dtc` does, in `guess_value_type`, so printed trees match its output:
/// strings if the value ends with a zero byte, holds nothing but printable characters,
/// zeros and escapable controls, and is at most half zeros. So `dtc` shows some numbers
/// as strings, such as `0x00070800` as `"", "\a\b"`.
pub fn shown(value: &[u8]) -> Shown<'_> {
    let printable = |byte: u8| (b' '..=b'~').contains(&byte);
    let stringish = |byte: u8| byte == 0 || printable(byte) || ESCAPED.iter().any(|&(control, _)| control == byte);
    let zeros = value.iter().filter(|&&byte| byte == 0).count();
    let looks_like_strings =
        value.last() == Some(&0) && value.iter().all(|&byte| stringish(byte)) && zeros <= value.len() - zeros;
    if value.is_empty() {
        Shown::Empty
    } else if looks_like_strings {
        Shown::Strings(value)
    } else if value.len().is_multiple_of(CELL_BYTES) {
        Shown::Cells(value)
    } else {
        Shown::Bytes(value)
    }
}

#[cfg(test)]
mod tests;
