//! Read-only census of Graphics/Game Unit properties used by ordinary R/C/I
//! buildings and service assets. It reports parent/category, model/lot refs,
//! path, UnitLocation and vehicle-bin columns so a package comparison does not
//! rely on visual inspection.

use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
use sc_registry::Registry;
use std::collections::BTreeMap;

const PROPERTY: u32 = 0x00B1_B104;
const UNIT_GROUP: u32 = 0x40E0_C000;
const MODEL: u32 = 0x2F4E_681B;
const H_PARENT: u32 = 0x00B2_CCCB;
const H_CATEGORY: u32 = 0x0AFE_27B1;
const H_CLASS: u32 = 0x099B_4C1F;
const H_BUNDLES: u32 = 0x0A02_09A5;
const H_LOT_LINK: u32 = 0x08E1_7ED7;
const H_VEHICLE_BASE: u32 = 0x0C12_EF20;
const H_VEHICLE_USE_PERCENT: u32 = 0x0C12_EF60;
const H_PATH_POINTS: u32 = 0x0CAA_680D;
const H_PATH_ENTRY: u32 = 0x0CC8_FE7A;
const H_LOCATION_IDS: u32 = 0x0E1B_AC61;
const H_LOCATION_XFORMS: u32 = 0x0E1B_AC62;

fn has(file: &PropertyFile, hash: u32) -> bool {
    file.get(hash)
        .is_some_and(|p| !matches!(p.kind, Kind::Empty))
}

fn key_instance(file: &PropertyFile, hash: u32) -> Option<u32> {
    let p = file.get(hash)?;
    let vals: Vec<&Value> = match &p.kind {
        Kind::Scalar(v) => vec![v],
        Kind::Array(vs) => vs.iter().collect(),
        Kind::Empty => vec![],
    };
    vals.into_iter().find_map(|v| match v {
        Value::Key(k) => Some(k.instance),
        _ => None,
    })
}

fn array_len(file: &PropertyFile, hash: u32) -> usize {
    file.get(hash).map_or(0, |p| match &p.kind {
        Kind::Array(v) => v.len(),
        Kind::Scalar(_) => 1,
        Kind::Empty => 0,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: rci_building_census <database_main.s3db> <package> [...]");
        std::process::exit(2);
    }
    let registry = Registry::open(&args[0]).expect("open registry");
    let mut total = 0usize;
    let mut classes: BTreeMap<char, usize> = BTreeMap::new();
    let mut feature_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for path in &args[1..] {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e:?}"));
        let mut rows = Vec::new();
        for e in package.entries() {
            if e.id.type_id != PROPERTY || (e.id.group & 0xFFFF) != (UNIT_GROUP & 0xFFFF) {
                continue;
            }
            let name = registry.instance_name(e.id.instance);
            let mut chars = name.chars();
            let Some(prefix) = chars.next() else { continue };
            if !matches!(prefix, 'R' | 'C' | 'I') || !name.starts_with(&format!("{prefix}_")) {
                continue;
            }
            let Ok(bytes) = package.read(e) else { continue };
            let Ok(file) = PropertyFile::parse(&bytes) else {
                continue;
            };
            total += 1;
            *classes.entry(prefix).or_default() += 1;
            let parent = key_instance(&file, H_PARENT).unwrap_or(0);
            let category = key_instance(&file, H_CATEGORY).unwrap_or(0);
            let class = key_instance(&file, H_CLASS).unwrap_or(0);
            let models = file
                .values
                .iter()
                .flat_map(|p| match &p.kind {
                    Kind::Scalar(Value::Key(k)) if k.type_id == MODEL => vec![k.instance],
                    Kind::Array(vs) => vs
                        .iter()
                        .filter_map(|v| match v {
                            Value::Key(k) if k.type_id == MODEL => Some(k.instance),
                            _ => None,
                        })
                        .collect(),
                    _ => vec![],
                })
                .collect::<Vec<_>>();
            let features = [
                ("parent", has(&file, H_PARENT)),
                ("category", has(&file, H_CATEGORY)),
                ("class", has(&file, H_CLASS)),
                ("bundles", has(&file, H_BUNDLES)),
                ("lot_link", has(&file, H_LOT_LINK)),
                ("vehicle_bin", has(&file, H_VEHICLE_BASE)),
                ("vehicle_percent", has(&file, H_VEHICLE_USE_PERCENT)),
                ("path_points", has(&file, H_PATH_POINTS)),
                ("path_entry", has(&file, H_PATH_ENTRY)),
                ("location_ids", has(&file, H_LOCATION_IDS)),
                ("location_xforms", has(&file, H_LOCATION_XFORMS)),
            ];
            for (label, present) in features {
                if present {
                    *feature_counts.entry(label).or_default() += 1;
                }
            }
            rows.push(format!(
                "{prefix} {name} I={:08X} parent={parent:08X} category={category:08X} class={class:08X} models={} bins={} binPercent={} paths={} entries={} locations={}/{}",
                e.id.instance,
                models.len(),
                array_len(&file, H_VEHICLE_BASE),
                has(&file, H_VEHICLE_USE_PERCENT),
                array_len(&file, H_PATH_POINTS),
                array_len(&file, H_PATH_ENTRY),
                array_len(&file, H_LOCATION_IDS),
                array_len(&file, H_LOCATION_XFORMS),
            ));
        }
        rows.sort();
        println!("=== {} ===", path);
        println!("RCI Unit rows = {}", rows.len());
        for row in rows {
            println!("{row}");
        }
    }
    println!("=== aggregate ===");
    println!("rows={total} classes={classes:?}");
    for (key, value) in feature_counts {
        println!("{key}={value}");
    }
}
