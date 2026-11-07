//! Find vanilla education menus (by locale title) and dump their Menu2/Unit
//! field tables, for behavior/presentation parity work.
//! Usage: cargo run --release -p fluffy-open-scp --example find_school_menu -- <scripts.package> [title-substring]

use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

const PROPERTY: u32 = 0x00B1_B104;
const MENU_GROUP: u32 = 0x0987_8A01;
const UNIT_GROUP: u32 = 0x40E0_C000;
const LOCALE_TYPE: u32 = 0x0A98_EAF0;
// Vanilla locale tables live with group 0 (OpenSCP's own tables use 02FABF01).
const LOCALE_GROUP: u32 = 0x0000_0000;
const HASH_TITLE: u32 = 0x0A09_F5FA;
const HASH_MENU_UNIT: u32 = 0x0975_695F;
const HASH_BUILDING_CATEGORY: u32 = 0x0975_695E;
const HASH_STAT_TEMPLATE: u32 = 0x0EB1_FC05;
const HASH_STAT_PARAMS: u32 = 0x0EB1_FC22;
const HASH_PARENT: u32 = 0x00B2_CCCB;
const HASH_UNIT_CATEGORY: u32 = 0x0AFE_27B1;
const HASH_CAPACITY: u32 = 0x0AD7_FAC9;

fn locale_tables(package: &Package) -> BTreeMap<u32, BTreeMap<String, String>> {
    let mut tables = BTreeMap::new();
    for entry in package.entries() {
        if entry.id.type_id != LOCALE_TYPE || entry.id.group != LOCALE_GROUP {
            continue;
        }
        let bytes = match package.read(entry) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        let text = match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
            Some(rest) => match std::str::from_utf8(rest) {
                Ok(text) => text,
                Err(_) => continue,
            },
            None => continue,
        };
        let value: serde_json::Value = match serde_json::from_slice(text.as_bytes()) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let mut rows = BTreeMap::new();
        if let Some(object) = value.as_object() {
            for (row, text) in object {
                if let Some(text) = text.as_str() {
                    rows.insert(row.clone(), text.to_string());
                }
            }
        }
        tables.insert(entry.id.instance, rows);
    }
    tables
}

fn menu_title<'a>(
    menu: &PropertyFile,
    tables: &'a BTreeMap<u32, BTreeMap<String, String>>,
) -> Option<&'a str> {
    let Kind::Array(values) = &menu.get(HASH_TITLE)?.kind else {
        return None;
    };
    let Value::Text(text) = values.first()? else {
        return None;
    };
    tables.get(&text.table_id)?.get(&text.instance_id.to_string()).map(String::as_str)
}

fn describe(file: &PropertyFile, hashes: &[u32]) {
    for hash in hashes {
        let Some(property) = file.get(*hash) else {
            continue;
        };
        let rendered = match &property.kind {
            Kind::Empty => "empty".into(),
            Kind::Scalar(value) => render(value),
            Kind::Array(values) => {
                values.iter().map(render).collect::<Vec<_>>().join(", ")
            }
        };
        println!("    {:08X} [{}] {}", hash, property.encoding.flags, rendered);
    }
}

fn render(value: &Value) -> String {
    match value {
        Value::Int32(v) => format!("i32:{v}"),
        Value::UInt32(v) => format!("u32:{v}"),
        Value::Float(v) => format!("f32:{v}"),
        Value::Bool(v) => format!("bool:{v}"),
        Value::Key(key) => format!("key({:08X},{:08X},{:08X})", key.type_id, key.group, key.instance),
        Value::Text(text) => format!("text({:08X},{})", text.table_id, text.instance_id),
        _ => format!("{value:?}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let needle = args.get(2).cloned().unwrap_or_else(|| "大學".into());
    let package = Package::open(std::path::Path::new(&args[1])).expect("open scripts");
    let tables = locale_tables(&package);
    println!("locale tables: {} {:?}", tables.len(), tables.keys().map(|k| format!("{k:08X}")).collect::<Vec<_>>());
    let menu_ids: Vec<_> = package
        .entries()
        .iter()
        .filter(|entry| entry.id.type_id == PROPERTY && entry.id.group == MENU_GROUP)
        .map(|entry| entry.id)
        .collect();
    println!("menus: {}", menu_ids.len());
    let mut matches = 0;
    for id in &menu_ids {
        let bytes = package.read(&package.entry(*id).expect("entry")).expect("read");
        let Ok(menu) = PropertyFile::parse(&bytes) else {
            continue;
        };
        // Mode: EDU_MENUS=1 dumps menus mounted on education containers.
        if std::env::var("EDU_MENUS").is_ok() {
            let edu = matches!(
                menu.get(0x0DB9_FC63).map(|p| &p.kind),
                Some(Kind::Array(values)) if values.iter().any(|v| matches!(v,
                    Value::Key(k) if k.instance == 0x6A17_85B4 || k.instance == 0x1E67_365A || k.instance == 0x95A9_D6FD
                ))
            );
            if edu {
                println!("=== EDU-MENU {:08X}", id.instance);
                describe(&menu, &[
                    HASH_MENU_UNIT, 0x0DB9_FC63, HASH_STAT_TEMPLATE, HASH_STAT_PARAMS,
                ]);
                if let Some(Kind::Scalar(Value::Key(unit_key))) =
                    menu.get(HASH_MENU_UNIT).map(|p| &p.kind)
                {
                    let unit_id = ResourceId {
                        type_id: PROPERTY,
                        group: UNIT_GROUP,
                        instance: unit_key.instance,
                    };
                    if let Some(entry) = package.entry(unit_id) {
                        if let Ok(unit) =
                            PropertyFile::parse(&package.read(entry).expect("read"))
                        {
                            describe(&unit, &[
                                HASH_PARENT, HASH_UNIT_CATEGORY, HASH_CAPACITY, 0x0A02_09A5,
                            ]);
                        }
                    }
                }
            }
            continue;
        }
        // Mode: DUMP_MENUUNIT=<hex> filters menus by their menuUnit target.
        if let Ok(target) = std::env::var("DUMP_MENUUNIT") {
            let target = u32::from_str_radix(target.trim_start_matches("0x"), 16).unwrap();
            let points_at_unit = matches!(
                menu.get(HASH_MENU_UNIT).map(|p| &p.kind),
                Some(Kind::Scalar(Value::Key(k))) if k.instance == target
            );
            if points_at_unit {
                println!(
                    "=== MENU {:08X}:{:08X}:{:08X} menuUnit->{target:08X}",
                    id.type_id, id.group, id.instance
                );
                describe(&menu, &[
                    HASH_MENU_UNIT, HASH_BUILDING_CATEGORY, HASH_STAT_TEMPLATE, HASH_STAT_PARAMS,
                    HASH_PARENT, 0x0DB9_FC63, 0x0977_AA8F, 0x0DDE_EE56, 0x0C65_D729, 0x0975_6955,
                    0x0EC4_80DF,
                ]);
            }
            continue;
        }
        let Some(title) = menu_title(&menu, &tables) else {
            if std::env::var("DUMP_TITLES").is_ok() {
                if let Some(Kind::Array(values)) = menu.get(HASH_TITLE).map(|p| &p.kind) {
                    if let Some(Value::Text(t)) = values.first() {
                        println!("NOTITLE {:08X} text({:08X},{})", id.instance, t.table_id, t.instance_id);
                    }
                }
            }
            continue;
        };
        if !title.contains(&needle) {
            if std::env::var("DUMP_TITLES").is_ok() {
                println!("TITLE {:08X} [{title}]", id.instance);
            }
            continue;
        }
        matches += 1;
        if matches > 6 {
            continue;
        }
        println!(
            "=== {} | menu {:08X}:{:08X}:{:08X} | title [{title}]",
            "EDU", id.type_id, id.group, id.instance
        );
        describe(
            &menu,
            &[
                HASH_MENU_UNIT,
                HASH_BUILDING_CATEGORY,
                HASH_STAT_TEMPLATE,
                HASH_STAT_PARAMS,
                HASH_PARENT,
            ],
        );
        // menuUnit -> Unit leaf
        if let Some(Kind::Scalar(Value::Key(unit_key))) = menu.get(HASH_MENU_UNIT).map(|p| &p.kind)
        {
            let unit_id = ResourceId {
                type_id: PROPERTY,
                group: UNIT_GROUP,
                instance: unit_key.instance,
            };
            if let Some(entry) = package.entry(unit_id) {
                if let Ok(unit) = PropertyFile::parse(&package.read(entry).expect("read")) {
                    println!("  UNIT {:08X}", unit_key.instance);
                    describe(
                        &unit,
                        &[
                            HASH_PARENT,
                            HASH_UNIT_CATEGORY,
                            HASH_CAPACITY,
                            0x0A02_09A2,
                            0x0A02_09A3,
                            0x0A02_09A4,
                            0x0A02_09A5,
                            0x0A02_09B2,
                            0x0EC4_80DF,
                        ],
                    );
                }
            }
        }
    }
    println!("matches: {matches}");
}
