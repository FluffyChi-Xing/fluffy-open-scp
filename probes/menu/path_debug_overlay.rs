//! Build a game-side probe for retained path/debug tools.
//!
//! The overlay puts one native `cToolPathSelector` entry and six retained
//! homeless-tent unit entries after slot 110 in the road palette. The tent
//! entries are cloned from a complete visible ploppable shell, then point their
//! `menuUnit` at the retail homeless-tent Unit (`EB415C8F`). The path entry is
//! cloned from a visible path placer and changes only its action to the retail
//! `cToolPathSelector` action (`C741981F`).
//!
//! ```text
//! cargo run -p probes --release --example path_debug_overlay -- \
//!   <SimCity_Game.package> <out.package> [start]
//! ```

use dbpf::{OverlayEntry, Package};
use sc_properties::debug_tools;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const MENU_GROUP: u32 = 0x0987_8A01;
const HASH_ACTION: u32 = 0x08F6_72C7;
const HASH_TITLE: u32 = 0x0A09_F5FA;
const HASH_DESC: u32 = 0x0A09_F5FB;
const HASH_MENU_UNIT: u32 = 0x0975_695F;
const HASH_UI_CATEGORY: u32 = 0x0DB9_FC63;
const HASH_UI_POSITION: u32 = 0x0DC1_E3E0;

const ROAD_CATEGORY: u32 = 0xAC12_C904;
const PATH_SHELL: u32 = 0x7DD8_A627;
const PATH_TARGET: u32 = 0xC741_981F;
const PATH_ACTION_OLD: u32 = 0x134C_DAC9;
const PATH_TITLE_OLD: u32 = 0x0E9F_6B70;
const PATH_DESC_OLD: u32 = 0x0E9F_6B71;
const PATH_TITLE: u32 = 0x0EC3_4920;
const PATH_DESC: u32 = 0x0EC3_4963;

const PLOP_SHELL: u32 = 0x75A9_0B66;
const PLOP_MENU_UNIT_OLD: u32 = 0xA9D6_7AA8;
const HOMELESS_UNIT: u32 = 0xEB41_5C8F;
const HOMELESS: &[(u32, u32, u32)] = &[
    (0x63A0_7C41, 0x0D14_1573, 0x0D14_1567),
    (0x63A2_BAA8, 0x0D14_1584, 0x0D14_1567),
    (0x239D_D8B6, 0x0D14_156E, 0x0D14_1567),
    (0x2393_4B12, 0x0D14_158E, 0x0D14_1567),
    (0x2395_8A7D, 0x0D14_1593, 0x0D14_1567),
    (0x6391_71CB, 0x0D14_1589, 0x0D14_1567),
];

fn entry<'a>(package: &'a Package, instance: u32) -> &'a dbpf::IndexEntry {
    package
        .entries()
        .iter()
        .find(|entry| {
            entry.id.type_id == PROPERTY_TYPE
                && entry.id.group == MENU_GROUP
                && entry.id.instance == instance
        })
        .unwrap_or_else(|| panic!("menu property 0x{instance:08X} not found"))
}

fn patch_text(data: &[u8], old: u32, new: u32, hash: u32) -> Vec<u8> {
    debug_tools::patch_property_refs(data, &[(hash, old, new)], |file, property| {
        debug_tools::text_instances(file, property)
    })
    .unwrap_or_else(|error| panic!("patch text 0x{hash:08X}: {error}"))
}

fn patch_key(data: &[u8], old: u32, new: u32, hash: u32) -> Vec<u8> {
    debug_tools::patch_key_property_instances(data, hash, old, new)
        .unwrap_or_else(|error| panic!("patch key 0x{hash:08X}: {error}"))
}

fn patch_positions(data: &[u8], old: i32, new: i32) -> Vec<u8> {
    let file = sc_properties::PropertyFile::parse(data).expect("parse position property");
    let property = file
        .get(HASH_UI_POSITION)
        .expect("path shell has no uiPosition");
    let sc_properties::Kind::Array(values) = &property.kind else {
        panic!("path shell uiPosition is not an array");
    };
    assert!(values.iter().all(|value| matches!(
        value,
        sc_properties::Value::Int32(v) if *v == old
    )));
    let hash = HASH_UI_POSITION.to_be_bytes();
    let hits: Vec<usize> = data
        .windows(4)
        .enumerate()
        .filter_map(|(i, window)| (window == hash).then_some(i))
        .collect();
    assert_eq!(hits.len(), 1, "uiPosition hash must occur once");
    let at = hits[0];
    let flags = u16::from_be_bytes([data[at + 6], data[at + 7]]);
    assert_ne!(flags & 0x30, 0, "uiPosition must be an array");
    let count = i32::from_be_bytes(data[at + 8..at + 12].try_into().unwrap()) as usize;
    let item_size = i32::from_be_bytes(data[at + 12..at + 16].try_into().unwrap()) as usize;
    assert_eq!(count, values.len());
    assert_eq!(item_size, 4);
    let mut patched = data.to_vec();
    for index in 0..count {
        let pos = at + 16 + index * item_size;
        patched[pos..pos + 4].copy_from_slice(&new.to_be_bytes());
    }
    let check = sc_properties::PropertyFile::parse(&patched).expect("parse patched position");
    assert!(matches!(
        check.get(HASH_UI_POSITION).map(|property| &property.kind),
        Some(sc_properties::Kind::Array(values)) if values.iter().all(|value| matches!(value, sc_properties::Value::Int32(v) if *v == new))
    ));
    patched
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 || args.len() > 3 {
        eprintln!("usage: path_debug_overlay <SimCity_Game.package> <out.package> [start]");
        std::process::exit(2);
    }
    let base = Package::open(&args[0]).expect("open SimCity_Game.package");
    let output = &args[1];
    let start = args
        .get(2)
        .map(|value| value.parse::<i32>().expect("start must be an integer"))
        .unwrap_or(110);
    let mut overlays = Vec::new();

    // Path selector: retain a complete path property shell and swap its
    // native action. The selector factory is present in the retail binary.
    let path_shell = base
        .read(entry(&base, PATH_SHELL))
        .expect("read path shell");
    let mut path = patch_key(&path_shell, PATH_ACTION_OLD, PATH_TARGET, HASH_ACTION);
    path = patch_text(&path, PATH_TITLE_OLD, PATH_TITLE, HASH_TITLE);
    path = patch_text(&path, PATH_DESC_OLD, PATH_DESC, HASH_DESC);
    path = patch_positions(&path, 5, start);
    overlays.push(OverlayEntry::new(entry(&base, PATH_TARGET).id, path));

    // Homeless tents: use a complete visible ploppable shell so the game has
    // all icon, marquee, category and position fields, then redirect only the
    // Unit and localized strings.
    let plop_shell = base
        .read(entry(&base, PLOP_SHELL))
        .expect("read plop shell");
    for (offset, (target, title, desc)) in HOMELESS.iter().enumerate() {
        let mut data = patch_key(
            &plop_shell,
            PLOP_MENU_UNIT_OLD,
            HOMELESS_UNIT,
            HASH_MENU_UNIT,
        );
        data = patch_text(&data, 0x7C9A_0071, *title, HASH_TITLE);
        data = patch_text(&data, 0xD8A3_9C91, *desc, HASH_DESC);
        data = patch_key(&data, 0x9D2E_F585, ROAD_CATEGORY, HASH_UI_CATEGORY);
        data = patch_positions(&data, 100, start + 1 + offset as i32);
        overlays.push(OverlayEntry::new(entry(&base, *target).id, data));
    }

    if let Some(parent) = std::path::Path::new(output).parent() {
        std::fs::create_dir_all(parent).expect("create output directory");
    }
    dbpf::write_uncompressed_overlay_to_path(output, &overlays).expect("write overlay");
    println!(
        "wrote {} entries to {output}: path selector slot {}, homeless tent slots {}-{}",
        overlays.len(),
        start,
        start + 1,
        start + HOMELESS.len() as i32
    );
    println!("path action C741981F -> cToolPathSelector; homeless Unit 40E1C000:EB415C8F");
}
