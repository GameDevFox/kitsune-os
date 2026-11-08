use std::io::Write;
use std::process::{Command, Stdio};

use super::*;

/// A Pi 4 tree as the firmware filled it in, from the old project. Its memory and
/// addresses match the live tree read from this Pi on 2026-11-07.
const PI4: &[u8] = include_bytes!("../../../kernel/dts/4-model-b.dtb");

/// Builds a tree from source with `dtc`, which must be installed.
fn compile(source: &str) -> Vec<u8> {
    compile_with(source, &[])
}

fn compile_with(source: &str, options: &[&str]) -> Vec<u8> {
    let mut dtc = Command::new("dtc")
        .args(["-I", "dts", "-O", "dtb", "-q"])
        .args(options)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("dtc must be installed for these tests");
    dtc.stdin.take().unwrap().write_all(source.as_bytes()).unwrap();
    let output = dtc.wait_with_output().unwrap();
    assert!(output.status.success(), "dtc failed");
    output.stdout
}

const SMALL: &str = r#"
/dts-v1/;
/ {
    #address-cells = <1>;
    #size-cells = <1>;
    model = "test";
    a {
        x = <1>;
        inner@10 { y = <2>; };
    };
    b@20 {
        x = <3>;
        empty;
        z = <4>;
    };
};
"#;

#[test]
fn a_real_tree_has_its_memory_where_the_firmware_said() {
    let tree = DeviceTree::new(PI4).unwrap();
    assert_eq!(tree.version(), 17);
    let (address_cells, size_cells) = (tree.cell("/", "#address-cells").unwrap(), tree.cell("/", "#size-cells").unwrap());
    let memory: Vec<_> = regions(tree.property("/memory", "reg").unwrap(), address_cells, size_cells).collect();
    assert_eq!(memory, [(0, 0x3B40_0000), (0x4000_0000, 0xBC00_0000)]);
}

#[test]
fn the_interrupt_controller_translates_to_the_arm_address() {
    let tree = DeviceTree::new(PI4).unwrap();
    let path = tree.symbol("gicv2").unwrap();
    assert_eq!(path, "/soc/interrupt-controller@40041000");
    let soc_cells = tree.cell("/soc", "#address-cells").unwrap();
    let soc_sizes = tree.cell("/soc", "#size-cells").unwrap();
    let (address, size) = regions(tree.property(path, "reg").unwrap(), soc_cells, soc_sizes).next().unwrap();
    assert_eq!((address, size), (0x4004_1000, 0x1000));
    let root_cells = tree.cell("/", "#address-cells").unwrap();
    let ranges = tree.property("/soc", "ranges").unwrap();
    assert_eq!(translate(address, ranges, soc_cells, root_cells, soc_sizes), Some(0xFF84_1000));
}

#[test]
fn a_whole_walk_balances_and_ends_cleanly() {
    let tree = DeviceTree::new(PI4).unwrap();
    let mut walk = tree.walk();
    assert_eq!(walk.next(), Some(Event::Node { name: "", depth: 0 }));
    let (mut nodes, mut ends) = (1, 0);
    for event in walk.by_ref() {
        match event {
            Event::Node { .. } => nodes += 1,
            Event::EndNode { .. } => ends += 1,
            Event::Property { .. } => {}
        }
    }
    assert!(!walk.broken());
    assert_eq!(nodes, ends);
    assert!(nodes > 100, "only {nodes} nodes");
}

#[test]
fn a_small_tree_walks_in_order() {
    let bytes = compile(SMALL);
    let tree = DeviceTree::new(&bytes).unwrap();
    let names: Vec<_> = tree
        .walk()
        .map(|event| match event {
            Event::Node { name, depth } => format!("node {name} {depth}"),
            Event::Property { name, depth, .. } => format!("property {name} {depth}"),
            Event::EndNode { depth } => format!("end {depth}"),
        })
        .collect();
    let expected = [
        "node  0", "property #address-cells 0", "property #size-cells 0", "property model 0",
        "node a 1", "property x 1", "node inner@10 2", "property y 2", "end 2", "end 1",
        "node b@20 1", "property x 1", "property empty 1", "property z 1", "end 1", "end 0",
    ];
    assert_eq!(names, expected);
}

#[test]
fn properties_are_found_by_path() {
    let bytes = compile(SMALL);
    let tree = DeviceTree::new(&bytes).unwrap();
    assert_eq!(tree.property("/", "model"), Some(&b"test\0"[..]));
    assert_eq!(tree.cell("/a", "x"), Some(1));
    // The same name in another node isn't confused with it.
    assert_eq!(tree.cell("/b", "x"), Some(3));
    assert_eq!(tree.cell("/b@20", "x"), Some(3));
    assert_eq!(tree.cell("/a/inner", "y"), Some(2));
    assert_eq!(tree.property("/b", "empty"), Some(&[][..]));
}

#[test]
fn what_isnt_there_isnt_found() {
    let bytes = compile(SMALL);
    let tree = DeviceTree::new(&bytes).unwrap();
    assert_eq!(tree.property("/a", "y"), None, "a child's property isn't its parent's");
    assert_eq!(tree.property("/inner", "y"), None, "a node is only found at its own depth");
    assert_eq!(tree.property("/b@30", "x"), None);
    assert_eq!(tree.property("/c", "x"), None);
    assert_eq!(tree.property("/a", "z"), None, "a later sibling's property isn't the node's");
    assert_eq!(tree.alias("anything"), None);
    assert_eq!(tree.symbol("anything"), None);
}

#[test]
fn a_node_of_the_right_name_but_the_wrong_depth_doesnt_count() {
    // `a` is inside `c`, so it isn't `/a`, and `/a/b` doesn't exist.
    let bytes = compile("/dts-v1/; / { c { a { }; b { p = <1>; }; }; };");
    let tree = DeviceTree::new(&bytes).unwrap();
    assert_eq!(tree.property("/a/b", "p"), None);
    assert_eq!(tree.cell("/c/b", "p"), Some(1));
}

#[test]
fn a_tree_cut_short_is_refused_even_within_its_blocks() {
    // Padding after the blocks, so only the total size shows the cut.
    const PADDING: &str = "64";
    let bytes = compile_with(SMALL, &["-p", PADDING]);
    assert!(DeviceTree::new(&bytes).is_some());
    assert!(DeviceTree::new(&bytes[..bytes.len() - 1]).is_none());
}

#[test]
fn a_bad_header_is_refused() {
    let mut bytes = compile(SMALL);
    assert!(DeviceTree::new(&bytes[..HEADER_BYTES - 1]).is_none());
    assert!(DeviceTree::new(&bytes[..bytes.len() - 1]).is_none(), "shorter than its total size");
    bytes[0] ^= 1;
    assert!(DeviceTree::new(&bytes).is_none(), "wrong magic");
}

/// The structure block's start, from the header.
fn structure_at(bytes: &[u8]) -> usize {
    u32::from_be_bytes(bytes[STRUCTURE_AT..STRUCTURE_AT + 4].try_into().unwrap()) as usize
}

#[test]
fn a_corrupt_token_ends_the_walk_as_broken() {
    let mut bytes = compile(SMALL);
    // The root's first property, just after its BEGIN_NODE and empty name.
    let at = structure_at(&bytes) + 8;
    assert_eq!(&bytes[at..at + 4], PROPERTY.to_be_bytes());
    bytes[at..at + 4].copy_from_slice(&0x55u32.to_be_bytes());
    let tree = DeviceTree::new(&bytes).unwrap();
    let mut walk = tree.walk();
    assert_eq!(walk.by_ref().count(), 1);
    assert!(walk.broken());
}

#[test]
fn a_property_running_past_the_structure_ends_the_walk_as_broken() {
    let mut bytes = compile(SMALL);
    let at = structure_at(&bytes) + 8;
    bytes[at + 4..at + 8].copy_from_slice(&0x0010_0000u32.to_be_bytes());
    let tree = DeviceTree::new(&bytes).unwrap();
    let mut walk = tree.walk();
    walk.by_ref().for_each(drop);
    assert!(walk.broken());
}

#[test]
fn values_are_shown_by_what_they_look_like() {
    assert_eq!(shown(b""), Shown::Empty);
    assert_eq!(shown(b"arm,gic-400\0"), Shown::Strings(b"arm,gic-400\0"));
    assert_eq!(shown(b"one\0two\0"), Shown::Strings(b"one\0two\0"));
    assert_eq!(shown(&[0, 0, 0, 1]), Shown::Cells(&[0, 0, 0, 1]));
    assert_eq!(shown(&[1, 2, 3]), Shown::Bytes(&[1, 2, 3]));
    // As `dtc` decides: at most half zeros can still be strings, even empty ones,
    // but more than half can't.
    assert_eq!(shown(&[0, 7, 8, 0]), Shown::Strings(&[0, 7, 8, 0]));
    assert_eq!(shown(b"a\0\0\0"), Shown::Cells(b"a\0\0\0"));
    assert_eq!(shown(&[0, 0, 0, 0]), Shown::Cells(&[0, 0, 0, 0]));
    assert_eq!(shown(&[0xFF, 0]), Shown::Bytes(&[0xFF, 0]), "not printable");
}

#[test]
fn translation_outside_every_range_fails() {
    let ranges: Vec<u8> = [0x7E00_0000u32, 0, 0xFE00_0000, 0x0180_0000].iter().flat_map(|cell| cell.to_be_bytes()).collect();
    assert_eq!(translate(0x7E20_1000, &ranges, 1, 2, 1), Some(0xFE20_1000));
    assert_eq!(translate(0x7F80_0000, &ranges, 1, 2, 1), None, "just past the range");
    assert_eq!(translate(0x1234, &[], 1, 2, 1), Some(0x1234));
}

#[test]
fn an_alias_gives_its_path() {
    let tree = DeviceTree::new(PI4).unwrap();
    assert_eq!(tree.alias("serial0"), Some("/soc/serial@7e201000"));
}

#[test]
fn a_tree_ending_inside_a_node_is_broken() {
    let bytes = compile(SMALL);
    let tree = DeviceTree::new(&bytes).unwrap();
    // The last END_NODE, the root's, just before END.
    let structure = structure_at(&bytes);
    let end = (structure..bytes.len() - 4).step_by(4).rev().find(|&at| bytes[at..at + 4] == END.to_be_bytes()).unwrap();
    let mut changed = bytes.clone();
    changed[end - 4..end].copy_from_slice(&NOP.to_be_bytes());
    let changed_tree = DeviceTree::new(&changed).unwrap();
    let mut walk = changed_tree.walk();
    walk.by_ref().for_each(drop);
    assert!(walk.broken());
    let mut whole = tree.walk();
    whole.by_ref().for_each(drop);
    assert!(!whole.broken());
}

/// `dtc`'s own source for a tree, without the labels it rebuilds from `/__symbols__`,
/// which `write_dts` leaves out.
fn dtc_source(bytes: &[u8]) -> String {
    let mut dtc = Command::new("dtc")
        .args(["-I", "dtb", "-O", "dts", "-q"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("dtc must be installed for these tests");
    dtc.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = dtc.wait_with_output().unwrap();
    assert!(output.status.success(), "dtc failed");
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| {
            let indent = line.len() - line.trim_start_matches('\t').len();
            let mut rest = &line[indent..];
            // A node line can start with labels, such as `soc: soc {`.
            while let Some((label, after)) = rest.split_once(": ") {
                if !rest.ends_with('{') || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    break;
                }
                rest = after;
            }
            format!("{}{rest}\n", &line[..indent])
        })
        .collect()
}

fn our_source(bytes: &[u8]) -> String {
    let mut text = String::new();
    write_dts(&DeviceTree::new(bytes).unwrap(), &mut text, || {}).unwrap();
    text
}

#[test]
fn a_real_tree_prints_as_dtc_prints_it() {
    let (ours, theirs) = (our_source(PI4), dtc_source(PI4));
    let different: Vec<_> = ours.lines().zip(theirs.lines()).filter(|(a, b)| a != b).take(5).collect();
    assert!(different.is_empty(), "first differences: {different:#?}");
    assert_eq!(ours.lines().count(), theirs.lines().count());
}

#[test]
fn a_small_tree_prints_as_dtc_prints_it() {
    let bytes = compile(SMALL);
    assert_eq!(our_source(&bytes), dtc_source(&bytes));
}

#[test]
fn every_line_is_followed_by_the_callback() {
    let tree = DeviceTree::new(PI4).unwrap();
    let (mut text, mut calls) = (String::new(), 0);
    write_dts(&tree, &mut text, || calls += 1).unwrap();
    assert_eq!(calls, text.lines().count());
}
