//! The MMU and data cache. Memory is mapped 1:1, apart from a second, uncached view of
//! the first block for anything the GPU also uses, which can't see into the cache.

use core::arch::asm;

use crate::board::MemoryType;

const TABLE_ENTRIES: usize = 512;
/// What each entry of the table covers, given the address space set in `TCR`.
const BLOCK_BYTES: u64 = 1 << 30;

/// In a table entry: this is a block rather than a pointer to another table.
const BLOCK: u64 = 0b01;
/// Without this, the first access to the block faults.
const ACCESS_FLAG: u64 = 1 << 10;
const INNER_SHAREABLE: u64 = 0b11 << 8;
const EXECUTE_NEVER: u64 = (1 << 53) | (1 << 54);
/// Which entry of `MAIR` the block uses.
const ATTRIBUTE_NORMAL: u64 = 0 << 2;
const ATTRIBUTE_DEVICE: u64 = 1 << 2;
const ATTRIBUTE_UNCACHED: u64 = 2 << 2;

/// Entry 0 is Normal, cached as write-back. Entry 1 is Device. Entry 2 is Normal, uncached.
const MAIR: u64 = 0x44_00_FF;
/// 39-bit addresses starting at level 1, 4 KB pages, walks cached, and no TTBR1.
const TCR: u64 = 25 | (0b01 << 8) | (0b01 << 10) | (0b11 << 12) | (1 << 23);
const SCTLR_MMU_AND_DATA_CACHE: u64 = (1 << 0) | (1 << 2);

#[repr(C, align(4096))]
struct Table([u64; TABLE_ENTRIES]);

static mut TABLE: Table = Table([0; TABLE_ENTRIES]);

fn block_entry(index: usize, memory: MemoryType) -> u64 {
    let own_address = index as u64 * BLOCK_BYTES;
    let (address, flags) = match memory {
        MemoryType::Normal => (own_address, ATTRIBUTE_NORMAL | INNER_SHAREABLE),
        MemoryType::UncachedFirstBlock => (0, ATTRIBUTE_UNCACHED | INNER_SHAREABLE | EXECUTE_NEVER),
        MemoryType::Device => (own_address, ATTRIBUTE_DEVICE | EXECUTE_NEVER),
    };
    address | BLOCK | ACCESS_FLAG | flags
}

/// Turns on the MMU and data cache, with the first blocks of memory mapped as `map`
/// says, and everything after them left unmapped, so that using it faults.
pub fn enable(map: &[MemoryType]) {
    // SAFETY: this core is the only thing running, nothing else uses the table, and
    // the code running now stays at the same address when the MMU turns on, as long
    // as `map` covers it. The barriers put the table in RAM before the MMU reads it.
    unsafe {
        let table = &raw mut TABLE;
        for (index, memory) in map.iter().enumerate() {
            (*table).0[index] = block_entry(index, *memory);
        }
        asm!(
            "dsb sy",
            "msr mair_el1, {mair}",
            "msr tcr_el1, {tcr}",
            "msr ttbr0_el1, {table}",
            "isb",
            "tlbi vmalle1",
            "dsb sy",
            "isb",
            "mrs {value}, sctlr_el1",
            "orr {value}, {value}, {mmu}",
            "msr sctlr_el1, {value}",
            "isb",
            mair = in(reg) MAIR,
            tcr = in(reg) TCR,
            table = in(reg) table as usize as u64,
            mmu = in(reg) SCTLR_MMU_AND_DATA_CACHE,
            value = out(reg) _,
            options(nostack),
        );
    }
}
