//! An ARM GIC-400, the GICv2 interrupt controller the Pi 4 has. Its distributor decides
//! which interrupts reach which core, and each core's CPU interface hands them over and
//! is told when one has been handled. Register offsets are from the GICv2 architecture
//! specification. The kernel runs non-secure, where the firmware has already put every
//! interrupt in group 1, so this only sees and enables that group.

use core::ptr::{read_volatile, write_volatile};

// Distributor registers.
const GICD_CTLR: usize = 0x000;
const GICD_TYPER: usize = 0x004;
const GICD_ISENABLER: usize = 0x100;
const GICD_ICENABLER: usize = 0x180;
const GICD_ICPENDR: usize = 0x280;
/// TYPER's low bits give how many 32-line groups the GIC has, less one.
const LINE_GROUPS_MASK: u32 = 0x1F;
const ALL_LINES: u32 = 0xFFFF_FFFF;
const GICD_IPRIORITYR: usize = 0x400;
/// One byte per interrupt, naming the cores it goes to, one bit each. The first eight
/// bytes, for private interrupts, are read-only and give the reading core's own bit.
const GICD_ITARGETSR: usize = 0x800;
/// Interrupts from here on are shared, and go to no core until targeted.
const FIRST_SHARED: u32 = 32;
/// In the non-secure view, bit 0 enables group 1.
const GICD_ENABLE: u32 = 1;

// CPU interface registers.
const GICC_CTLR: usize = 0x000;
const GICC_PMR: usize = 0x004;
const GICC_IAR: usize = 0x00C;
const GICC_EOIR: usize = 0x010;
const GICC_ENABLE: u32 = 1;
/// The lowest priority: let everything through.
const ALLOW_ALL_PRIORITIES: u32 = 0xFF;

/// The interrupt number's bits in IAR and EOIR. The bits above name the core that sent
/// a software interrupt, and must be written back unchanged.
const INTERRUPT_ID_MASK: u32 = 0x3FF;
/// What IAR gives when nothing is pending after all.
pub const SPURIOUS: u32 = 1023;

const BITS_PER_WORD: u32 = 32;
const WORD_BYTES: usize = 4;

#[derive(Clone, Copy)]
pub struct Gic {
    distributor: usize,
    cpu_interface: usize,
}

/// One acknowledged interrupt, which `Gic::end` must be told about.
pub struct Acknowledged(u32);

impl Acknowledged {
    pub fn id(&self) -> u32 {
        self.0 & INTERRUPT_ID_MASK
    }
}

impl Gic {
    /// `distributor` and `cpu_interface` must be the GIC's registers, in device memory.
    pub const fn new(distributor: usize, cpu_interface: usize) -> Self {
        Self { distributor, cpu_interface }
    }

    fn write(&self, address: usize, value: u32) {
        // SAFETY: the caller of `new` vouched for these registers.
        unsafe { write_volatile(address as *mut u32, value) };
    }

    fn read(&self, address: usize) -> u32 {
        // SAFETY: as in `write`.
        unsafe { read_volatile(address as *const u32) }
    }

    /// Turns on the distributor and this core's interface, letting every priority in,
    /// with every line disabled and nothing pending, so that only lines enabled from now
    /// on can arrive. A JTAG reload keeps the old kernel's GIC and devices as they were:
    /// on 2026-11-07, its heartbeat's alarm arrived before the new kernel had a handler
    /// for it, and with nothing to clear it, it arrived again forever.
    pub fn start(&self) {
        let groups = (self.read(self.distributor + GICD_TYPER) & LINE_GROUPS_MASK) as usize + 1;
        for group in 0..groups {
            self.write(self.distributor + GICD_ICENABLER + group * WORD_BYTES, ALL_LINES);
            self.write(self.distributor + GICD_ICPENDR + group * WORD_BYTES, ALL_LINES);
        }
        self.write(self.distributor + GICD_CTLR, GICD_ENABLE);
        self.write(self.cpu_interface + GICC_PMR, ALLOW_ALL_PRIORITIES);
        self.write(self.cpu_interface + GICC_CTLR, GICC_ENABLE);
    }

    /// Lets one interrupt through to this core, at a priority where lower is more urgent.
    /// A shared interrupt is also sent to this core: found on 2026-11-07, the system
    /// timer's targeted no core at all, so it was pending but never delivered.
    pub fn enable(&self, id: u32, priority: u8) {
        let priority_at = self.distributor + GICD_IPRIORITYR + id as usize;
        // SAFETY: priorities and targets are one byte per interrupt, and can be accessed
        // as bytes. The first target byte is this core's own bit.
        unsafe {
            write_volatile(priority_at as *mut u8, priority);
            if id >= FIRST_SHARED {
                let this_core = read_volatile((self.distributor + GICD_ITARGETSR) as *const u8);
                write_volatile((self.distributor + GICD_ITARGETSR + id as usize) as *mut u8, this_core);
            }
        }
        let word = self.distributor + GICD_ISENABLER + (id / BITS_PER_WORD) as usize * WORD_BYTES;
        self.write(word, 1 << (id % BITS_PER_WORD));
    }

    /// Stops one interrupt reaching any core.
    pub fn disable(&self, id: u32) {
        let word = self.distributor + GICD_ICENABLER + (id / BITS_PER_WORD) as usize * WORD_BYTES;
        self.write(word, 1 << (id % BITS_PER_WORD));
    }

    /// Takes the most urgent pending interrupt, which may be `SPURIOUS`.
    pub fn acknowledge(&self) -> Acknowledged {
        Acknowledged(self.read(self.cpu_interface + GICC_IAR))
    }

    /// Tells the GIC an interrupt has been handled, so it can send it again.
    pub fn end(&self, interrupt: Acknowledged) {
        self.write(self.cpu_interface + GICC_EOIR, interrupt.0);
    }
}
