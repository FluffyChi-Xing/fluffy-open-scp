//! Build a minimal terrain brush overlay for an in-game round trip.
//!
//! The retail executable still runs `RunScriptBrushes` when a region brush
//! property changes.  This probe swaps one region's `waterTableEcoMapBrushes`
//! key for another retail typed-map effect, leaving transforms and every other
//! resource untouched.  It is deliberately one property/one region so a game
//! test can tell whether the execution and 341-map write-back path is alive.
//!
//! ```text
//! cargo run -p probes --release --example terrain_water_table_validation -- \
//!   <SimCityData> <out.package> [target-region-group]
//! ```

use std::path::{Path, PathBuf};

use dbpf::{OverlayEntry, Package};
use sc_properties::{debug_tools, Kind, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const NAME: u32 = 0x00B2_CCCA;
const BRUSH_KEYS: u32 = 0x02A9_07B5;
const HEIGHTMAP_NAME: u32 = 0x0DBA_3A9C;
const RESOLUTION: u32 = 0x0DC0_97E3;
const HEIGHTMAP_TYPE: u32 = 0x0DE4_3899;

fn packages(root: &Path) -> Vec<PathBuf> {
    let mut paths = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|v| v.to_str()) == Some("package"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn name(file: &PropertyFile) -> Option<&str> {
    match &file.get(NAME)?.kind {
        Kind::Scalar(Value::String8(value)) | Kind::Scalar(Value::String16(value)) => {
            Some(value.as_str())
        }
        _ => None,
    }
}

fn first_key(file: &PropertyFile) -> Option<u32> {
    match &file.get(BRUSH_KEYS)?.kind {
        Kind::Array(values) => values.iter().find_map(|value| match value {
            Value::Key(key) => Some(key.instance),
            _ => None,
        }),
        Kind::Scalar(Value::Key(key)) => Some(key.instance),
        _ => None,
    }
}

fn scalar_string<'a>(file: &'a PropertyFile, hash: u32) -> Option<&'a str> {
    match &file.get(hash)?.kind {
        Kind::Scalar(Value::String8(value)) | Kind::Scalar(Value::String16(value)) => Some(value.as_str()),
        _ => None,
    }
}

fn scalar_u32(file: &PropertyFile, hash: u32) -> Option<u32> {
    match &file.get(hash)?.kind {
        Kind::Scalar(Value::UInt32(value)) => Some(*value),
        Kind::Scalar(Value::Int32(value)) if *value >= 0 => Some(*value as u32),
        _ => None,
    }
}

fn parse_hex(value: Option<&String>) -> Option<u32> {
    value.and_then(|value| u32::from_str_radix(value.trim_start_matches("0x"), 16).ok())
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let root = PathBuf::from(args.first().expect("usage: terrain_water_table_validation <SimCityData> <out.package> [group]"));
    let out = PathBuf::from(args.get(1).expect("missing output package"));
    let target_group = parse_hex(args.get(2));

    let mut target: Option<(dbpf::ResourceId, Vec<u8>, u32, String)> = None;
    let mut candidates = Vec::new();
    for path in packages(&root) {
        let Ok(package) = Package::open(&path) else { continue };
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY_TYPE { continue; }
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else { continue };
            if name(&file) != Some("waterTableEcoMapBrushes") { continue; }
            let Some(key) = first_key(&file) else { continue };
            candidates.push((entry.id.group, entry.id.instance, key, path.clone()));
            if target_group.is_some_and(|group| group != entry.id.group) { continue; }
            if target.is_none() {
                target = Some((entry.id, data, key, path.display().to_string()));
            }
        }
    }
    candidates.sort_by_key(|row| row.0);
    let (target_id, target_data, old_key, source_path) = target.expect("no waterTableEcoMapBrushes target found");
    let target_file = PropertyFile::parse(&target_data).expect("parse target property");
    let (source_group, _source_instance, new_key, source_brush_path) = candidates
        .iter()
        .find(|(group, _instance, key, _path)| *group != target_id.group && *key != old_key)
        .cloned()
        .expect("no alternate retail water-table brush found");
    let patched = debug_tools::patch_key_property_instances(&target_data, BRUSH_KEYS, old_key, new_key)
        .expect("patch waterTableEcoMapBrushes key");
    let entry = OverlayEntry::new(target_id, patched.clone());
    if let Some(parent) = out.parent() { std::fs::create_dir_all(parent).ok(); }
    std::fs::write(&out, dbpf::write_uncompressed_overlay(&[entry]).expect("encode overlay"))
        .expect("write overlay");

    println!("target TGI={:08X}:{:08X}:{:08X} source={source_path}", target_id.type_id, target_id.group, target_id.instance);
    println!("typed-map basename={:?} resolution={:?} map_type={:?}", scalar_string(&target_file, HEIGHTMAP_NAME), scalar_u32(&target_file, RESOLUTION), scalar_u32(&target_file, HEIGHTMAP_TYPE));
    println!("waterTableEcoMapBrushes old=0x{old_key:08X} new=0x{new_key:08X} candidate_group=0x{source_group:08X} candidate_source={}", source_brush_path.display());
    println!("overlay={} bytes={} (one property resource)", out.display(), patched.len());
    println!("round-trip: start a fresh city in region group 0x{:08X}, capture water table, then compare 341 eco-map tiles after save/restart", target_id.group);
}
