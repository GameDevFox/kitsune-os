//! What the firmware's device tree says about this board. The tree is found again for
//! each use, from the pointer `boot.s` saved, as finding it only reads its header.

use core::slice;

use device_tree::{cells, regions, translate, write_dts, DeviceTree, HEADER_BYTES};

use crate::board;
use crate::console;

/// Prints the whole tree over serial, in the source form `dtc` writes.
pub const PRINT_KEY: u8 = b'9';
/// A tree's header must be aligned to 8 bytes.
const ALIGNMENT: usize = 8;

/// The tree, if the saved pointer leads to one lying wholly in the CPU's RAM. RAM
/// survives a reboot, so the pointer could be left over from anything.
pub fn find() -> Option<DeviceTree<'static>> {
    let address = board::saved_device_tree_pointer();
    let end = board::CPU_MEMORY_END;
    if address == 0 || !address.is_multiple_of(ALIGNMENT) || address.checked_add(HEADER_BYTES)? > end {
        return None;
    }
    // SAFETY: the header lies in RAM, checked above.
    let header = unsafe { slice::from_raw_parts(address as *const u8, HEADER_BYTES) };
    let size = device_tree::total_size(header)?;
    if address.checked_add(size)? > end {
        return None;
    }
    // SAFETY: the whole tree lies in RAM, checked above, and nothing writes over it.
    DeviceTree::new(unsafe { slice::from_raw_parts(address as *const u8, size) })
}

/// The path of the node holding `path`, such as `/soc` for `/soc/serial@7e201000`.
fn parent(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) | None => "/",
        Some(at) => &path[..at],
    }
}

/// The CPU's address for one region of a node's `reg`, counting from 0. Its parent's
/// `ranges` translate it, which covers nodes on a bus such as `/soc`, but not buses
/// inside buses.
fn cpu_address(tree: &DeviceTree, path: &str, region: usize) -> Option<u64> {
    let bus = parent(path);
    let cells = |path: &str| Some((tree.cell(path, "#address-cells")?, tree.cell(path, "#size-cells")?));
    let (address_cells, size_cells) = cells(bus)?;
    let (address, _) = regions(tree.property(path, "reg")?, address_cells, size_cells).nth(region)?;
    if bus == "/" {
        return Some(address);
    }
    let (root_cells, _) = cells(parent(bus))?;
    translate(address, tree.property(bus, "ranges")?, address_cells, root_cells, size_cells)
}

/// Prints what the tree says that the kernel otherwise has written in by hand.
pub fn report() {
    let Some(tree) = find() else {
        println!("No device tree: the saved pointer is {:#x}.", board::saved_device_tree_pointer());
        return;
    };
    println!("Device tree at {:#x}, {} bytes, version {}.", board::saved_device_tree_pointer(), tree.size(), tree.version());
    if let (Some(memory), Some(address_cells), Some(size_cells)) =
        (tree.property("/memory", "reg"), tree.cell("/", "#address-cells"), tree.cell("/", "#size-cells"))
    {
        print!("Memory:");
        for (start, size) in regions(memory, address_cells, size_cells) {
            print!(" {start:#x} to {:#x},", start + size);
        }
        println!(" by the device tree.");
    }
    match interrupt_controller_in(&tree) {
        Some((distributor, _)) => println!("Interrupt controller at {distributor:#x}, by the device tree."),
        None => println!("The device tree doesn't say where the interrupt controller is."),
    }
}

// The GIC's `reg` lists its distributor's registers, then its CPU interface's.
const DISTRIBUTOR_REGION: usize = 0;
const CPU_INTERFACE_REGION: usize = 1;

fn interrupt_controller_in(tree: &DeviceTree) -> Option<(usize, usize)> {
    let path = tree.symbol("gicv2")?;
    let distributor = cpu_address(tree, path, DISTRIBUTOR_REGION)?;
    let cpu_interface = cpu_address(tree, path, CPU_INTERFACE_REGION)?;
    Some((usize::try_from(distributor).ok()?, usize::try_from(cpu_interface).ok()?))
}

/// The CPU's addresses for the GIC's distributor and CPU interface.
pub fn interrupt_controller() -> Option<(usize, usize)> {
    interrupt_controller_in(&find()?)
}

// A GIC interrupt is described in three cells: its type, its number within that type,
// and flags. Private interrupts, one set per core, are numbered from 16, and shared
// ones from 32.
const INTERRUPT_CELLS: usize = 3;
const TYPE_SHARED: u32 = 0;
const TYPE_PRIVATE: u32 = 1;
const FIRST_PRIVATE: u32 = 16;
const FIRST_SHARED: u32 = 32;
/// The ARM timer's interrupts come in a fixed order: secure physical, non-secure
/// physical, virtual, then hypervisor.
const VIRTUAL_TIMER: usize = 2;

/// The GIC's number for one of a node's interrupts, counting from 0.
fn interrupt_id(tree: &DeviceTree, path: &str, index: usize) -> Option<u32> {
    let mut specifier = cells(tree.property(path, "interrupts")?).skip(index * INTERRUPT_CELLS);
    let (kind, number) = (specifier.next()?, specifier.next()?);
    match kind {
        TYPE_PRIVATE => Some(FIRST_PRIVATE + number),
        TYPE_SHARED => Some(FIRST_SHARED + number),
        _ => None,
    }
}

/// The GIC's number for the core's virtual timer interrupt.
pub fn virtual_timer_interrupt() -> Option<u32> {
    interrupt_id(&find()?, "/timer", VIRTUAL_TIMER)
}

/// The system timer's address, and the GIC's number for one channel's interrupt, as the
/// node lists one interrupt per channel.
pub fn system_timer(channel: usize) -> Option<(usize, u32)> {
    let tree = find()?;
    let path = tree.symbol("system_timer")?;
    let base = usize::try_from(cpu_address(&tree, path, 0)?).ok()?;
    Some((base, interrupt_id(&tree, path, channel)?))
}

/// Prints the whole tree over serial only, as it's thousands of lines. Serial input is
/// read between lines, so that it isn't lost while this runs, which takes a while.
pub fn print_all() {
    match find() {
        Some(tree) => {
            let _ = write_dts(&tree, &mut console::Serial, console::echo_pending_input);
        }
        None => println!("No device tree to print."),
    }
}

/// The CPU's address for the first region of the node a label in `/__symbols__` names,
/// such as `gpio`.
pub fn address_of(label: &str) -> Option<usize> {
    let tree = find()?;
    usize::try_from(cpu_address(&tree, tree.symbol(label)?, 0)?).ok()
}

/// The temperature sensor's conversion from `/thermal-zones/cpu-thermal`: thousandths of
/// a degree are the first number times the raw reading, plus the second.
pub fn thermal_conversion() -> Option<(i32, i32)> {
    let tree = find()?;
    let mut numbers = cells(tree.property("/thermal-zones/cpu-thermal", "coefficients")?);
    // The cells hold signed numbers, as the slope is negative.
    Some((numbers.next()? as i32, numbers.next()? as i32))
}

/// A `gpios` entry is the controller's phandle, the pin, then flags, where bit 0 means
/// the pin is active low.
const GPIO_PIN_CELL: usize = 1;
const GPIO_ACTIVE_LOW: u32 = 1;

/// The green activity LED's pin, and whether it lights when low.
pub fn activity_led() -> Option<(u32, bool)> {
    let tree = find()?;
    let mut entry = cells(tree.property("/leds/led-act", "gpios")?).skip(GPIO_PIN_CELL);
    let (pin, flags) = (entry.next()?, entry.next()?);
    Some((pin, flags & GPIO_ACTIVE_LOW != 0))
}
