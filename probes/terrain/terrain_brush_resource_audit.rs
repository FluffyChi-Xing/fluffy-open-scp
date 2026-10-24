//! Audit the hidden Terrain Brush selector and resolve its brush names to DBPF resources.
//!
//! Usage:
//!   cargo run -p probes --release --example terrain_brush_resource_audit -- <SimCityData>
//!
//! This is deliberately read-only.  It does not infer that a menu item is
//! executable; it only proves whether the selector's brush keys resolve to
//! property resources and records their typed-map metadata.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dbpf::Package;
use sc_properties::{Kind, Key, PropertyFile, Value};

const MENU_TYPE: u32 = 0x00B1_B104;
const MENU_GROUP: u32 = 0x0987_8A01;
const MENU_INSTANCE: u32 = 0x4540_41B4;
const BRUSH_KEYS: u32 = 0x0D79_C8ED;
const BRUSH_NAMES: u32 = 0x0D79_C8EE;
const NAME: u32 = 0x00B2_CCCA;
const RESOURCE_KEY: u32 = 0x02A9_07B5;
const HEIGHTMAP_NAME: u32 = 0x0DBA_3A9C;
const HEIGHTMAP_TYPE: u32 = 0x0DE4_3899;

const REGION_BRUSH_NAMES: [&str; 10] = [
    "terrain", "water", "forest", "desire", "soil", "oil", "ore", "coal",
    "radiation", "groundpollution",
];

fn packages(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let Ok(rd) = std::fs::read_dir(root) else { return paths };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) == Some("package") {
            paths.push(path);
        }
    }
    paths.sort();
    paths
}

fn scalar_string(file: &PropertyFile, hash: u32) -> Option<String> {
    match file.get(hash)?.kind {
        Kind::Scalar(Value::String8(ref s)) | Kind::Scalar(Value::String16(ref s)) => Some(s.clone()),
        _ => None,
    }
}

fn key_values(file: &PropertyFile, hash: u32) -> Vec<Key> {
    let Some(p) = file.get(hash) else { return Vec::new() };
    match &p.kind {
        Kind::Scalar(Value::Key(k)) => vec![*k],
        Kind::Array(values) => values.iter().filter_map(|v| match v { Value::Key(k) => Some(*k), _ => None }).collect(),
        Kind::Empty | Kind::Scalar(_) => Vec::new(),
    }
}

fn string_values(file: &PropertyFile, hash: u32) -> Vec<String> {
    let Some(p) = file.get(hash) else { return Vec::new() };
    match &p.kind {
        Kind::Scalar(Value::String8(s)) | Kind::Scalar(Value::String16(s)) => vec![s.clone()],
        Kind::Array(values) => values.iter().filter_map(|v| match v {
            Value::String8(s) | Value::String16(s) => Some(s.clone()),
            _ => None,
        }).collect(),
        Kind::Empty | Kind::Scalar(_) => Vec::new(),
    }
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("SimCityData"));
    let mut menu: Option<PropertyFile> = None;
    let mut files = Vec::new();
    for path in packages(&root) {
        let Ok(package) = Package::open(&path) else { continue };
        for entry in package.entries() {
            if entry.id.type_id == MENU_TYPE && entry.id.group == MENU_GROUP && entry.id.instance == MENU_INSTANCE {
                if let Ok(bytes) = package.read(entry) { menu = PropertyFile::parse(&bytes).ok(); }
            }
        }
        files.push((path, package));
    }
    let Some(menu) = menu else { panic!("hidden terrain menu not found") };
    let keys = key_values(&menu, BRUSH_KEYS);
    let names = string_values(&menu, BRUSH_NAMES);
    println!("menu={MENU_TYPE:08X}:{MENU_GROUP:08X}:{MENU_INSTANCE:08X} keys={} names={}", keys.len(), names.len());
    let mut by_instance: HashMap<u32, Vec<(PathBuf, u32, u32, u32, usize, Option<String>)>> = HashMap::new();
    for (path, package) in &files {
        for entry in package.entries() {
            if keys.iter().any(|key| key.instance == entry.id.instance) {
                let bytes = package.read(entry).unwrap_or_default();
                let parsed = PropertyFile::parse(&bytes).ok();
                let label = parsed.as_ref().and_then(|f| scalar_string(f, 0x00B2_CCCA));
                by_instance.entry(entry.id.instance).or_default().push((path.clone(), entry.id.type_id, entry.id.group, entry.id.instance, bytes.len(), label));
            }
        }
    }
    for (index, key) in keys.iter().enumerate() {
        let name = names.get(index).map(String::as_str).unwrap_or("<missing-name>");
        let hits = by_instance.get(&key.instance);
        if let Some(hits) = hits {
            println!("{index:03} {name:<58} key=T{:08X}:G{:08X}:I{:08X} hits={}", key.type_id, key.group, key.instance, hits.len());
            for (path, ty, group, instance, size, label) in hits.iter().take(4) {
                println!("      {:08X}:{:08X}:{:08X} {:>8}B label={:?} file={}", ty, group, instance, size, label, path.display());
            }
        } else {
            println!("{index:03} {name:<58} key=T{:08X}:G{:08X}:I{:08X} hits=0", key.type_id, key.group, key.instance);
        }
    }

    // The retail region descriptors contain the typed-map brush references,
    // but the property list that cTerrainRegion::Init would load for each
    // reference is a separate resource in the same region group.  Report the
    // closure explicitly: this is the decisive distinction between a catalog
    // entry and an executable brush.
    let mut definitions: HashMap<(u32, u32), Vec<(u32, usize, Option<String>)>> = HashMap::new();
    for (path, package) in &files {
        for entry in package.entries() {
            let Ok(bytes) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&bytes) else { continue };
            let label = scalar_string(&file, NAME);
            definitions.entry((entry.id.group, entry.id.instance)).or_default().push((entry.id.type_id, bytes.len(), label));
            let _ = path;
        }
    }
    println!("\nregion brush definition closure (B1B104 instance + region group):");
    for (path, package) in &files {
        for entry in package.entries() {
            if entry.id.type_id != MENU_TYPE { continue; }
            let Ok(bytes) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&bytes) else { continue };
            let Some(name) = scalar_string(&file, NAME) else { continue };
            if !name.ends_with("EcoMapBrushes") { continue; }
            let Some(Kind::Array(values)) = file.get(RESOURCE_KEY).map(|p| &p.kind) else { continue };
            let Some(Value::Key(key)) = values.first() else { continue };
            let resolved = definitions.get(&(entry.id.group, key.instance));
            let kind = name.strip_suffix("EcoMapBrushes").unwrap_or_default();
            let slot = REGION_BRUSH_NAMES.iter().position(|x| x.eq_ignore_ascii_case(kind)).map(|x| x.to_string()).unwrap_or_else(|| kind.to_string());
            println!("  group={:08X} slot={slot:<14} descriptor={:08X} key={:08X} payload={} definition_count={} source={}", entry.id.group, entry.id.instance, key.instance, bytes.len(), resolved.map(|v| v.len()).unwrap_or(0), path.display());
        }
    }

    // Typed-map scripts use the same property hash for their child effect IDs.
    // Keep a second count so a future development package can be recognized by
    // the presence of the child effect resources, rather than by menu names.
    let mut typed_rows = 0usize;
    let mut child_refs = 0usize;
    let mut missing_children = 0usize;
    let mut samples = Vec::new();
    for (path, package) in &files {
        for entry in package.entries() {
            if entry.id.type_id != MENU_TYPE { continue; }
            let Ok(bytes) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&bytes) else { continue };
            let Some(basename) = scalar_string(&file, HEIGHTMAP_NAME) else { continue };
            let Some(Kind::Scalar(Value::UInt32(map_type))) = file.get(HEIGHTMAP_TYPE).map(|p| &p.kind) else { continue };
            if *map_type == 0 || *map_type > 11 { continue; }
            let children = key_values(&file, RESOURCE_KEY);
            if children.is_empty() { continue; }
            typed_rows += 1;
            for child in children {
                child_refs += 1;
                if !definitions.contains_key(&(entry.id.group, child.instance)) {
                    missing_children += 1;
                    if samples.len() < 12 {
                        samples.push(format!("{}:{:08X}:{:08X}->{:08X}", basename, entry.id.group, entry.id.instance, child.instance));
                    }
                }
            }
            let _ = path;
        }
    }
    println!("\ntyped-map script child-effect closure: rows={typed_rows} refs={child_refs} missing={missing_children}");
    for sample in samples { println!("  missing-child {sample}"); }
}
