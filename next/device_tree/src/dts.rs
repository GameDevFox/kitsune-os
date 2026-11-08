//! Writes a tree as source, in the layout `dtc -I dtb -O dts` uses, apart from labels,
//! which `dtc` rebuilds from `/__symbols__`. Matching it lets the tests compare the two
//! line by line.

use core::fmt::{self, Write};

use crate::{cells, read_u32, shown, DeviceTree, Event, Shown, ESCAPED};

const RESERVATIONS_AT: usize = 16;
const RESERVATION_BYTES: usize = 16;
const HALF_BYTES: usize = 8;

/// Writes the tree, calling `between_lines` after each line, which lets a slow output
/// keep other work going.
pub fn write(tree: &DeviceTree, out: &mut impl Write, mut between_lines: impl FnMut()) -> fmt::Result {
    let mut line = |out: &mut dyn Write, args: fmt::Arguments| -> fmt::Result {
        out.write_fmt(args)?;
        out.write_char('\n')?;
        between_lines();
        Ok(())
    };
    line(out, format_args!("/dts-v1/;"))?;
    line(out, format_args!(""))?;
    for (address, size) in reservations(tree) {
        line(out, format_args!("/memreserve/\t{address:#018x} {size:#018x};"))?;
    }
    let mut walk = tree.walk();
    for event in walk.by_ref() {
        match event {
            Event::Node { name, depth } => {
                if depth > 0 {
                    line(out, format_args!(""))?;
                }
                let name = if depth == 0 { "/" } else { name };
                line(out, format_args!("{}{name} {{", Indent(depth)))?;
            }
            Event::EndNode { depth } => line(out, format_args!("{}}};", Indent(depth)))?,
            Event::Property { name, value, depth } => {
                line(out, format_args!("{}{name}{};", Indent(depth + 1), Value(value)))?;
            }
        }
    }
    if walk.broken() {
        line(out, format_args!("// The tree is malformed past this point."))?;
    }
    Ok(())
}

/// The memory reservation block's address and size pairs, which end at a pair of zeros.
fn reservations<'a>(tree: &DeviceTree<'a>) -> impl Iterator<Item = (u64, u64)> + 'a {
    let bytes = tree.bytes;
    let start = read_u32(bytes, RESERVATIONS_AT).map_or(bytes.len(), |at| at as usize);
    let entries = bytes.get(start..).unwrap_or(&[]).chunks_exact(RESERVATION_BYTES);
    entries
        .map(|entry| {
            let half = |at: usize| u64::from_be_bytes(entry[at..at + HALF_BYTES].try_into().unwrap_or_default());
            (half(0), half(HALF_BYTES))
        })
        .take_while(|&pair| pair != (0, 0))
}

struct Indent(usize);

impl fmt::Display for Indent {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        (0..self.0).try_for_each(|_| out.write_char('\t'))
    }
}

/// A property's value, with the ` = ` before it, or nothing for an empty one.
struct Value<'a>(&'a [u8]);

impl fmt::Display for Value<'_> {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        match shown(self.0) {
            Shown::Empty => Ok(()),
            Shown::Strings(strings) => {
                out.write_str(" = ")?;
                let text = &strings[..strings.len() - 1];
                for (index, part) in text.split(|&byte| byte == 0).enumerate() {
                    if index > 0 {
                        out.write_str(", ")?;
                    }
                    out.write_char('"')?;
                    for &byte in part {
                        if let Some(&(_, letter)) = ESCAPED.iter().find(|&&(control, _)| control == byte) {
                            write!(out, "\\{letter}")?;
                        } else {
                            if byte == b'"' || byte == b'\\' {
                                out.write_char('\\')?;
                            }
                            out.write_char(char::from(byte))?;
                        }
                    }
                    out.write_char('"')?;
                }
                Ok(())
            }
            Shown::Cells(value) => {
                out.write_str(" = <")?;
                for (index, cell) in cells(value).enumerate() {
                    if index > 0 {
                        out.write_char(' ')?;
                    }
                    write!(out, "{cell:#04x}")?;
                }
                out.write_char('>')
            }
            Shown::Bytes(value) => {
                out.write_str(" = [")?;
                for (index, byte) in value.iter().enumerate() {
                    if index > 0 {
                        out.write_char(' ')?;
                    }
                    write!(out, "{byte:02x}")?;
                }
                out.write_char(']')
            }
        }
    }
}
