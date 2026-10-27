//! Audit the hidden Terrain Brush selector and resolve its brush names to DBPF resources.
//!
//! Usage:
//!   cargo run -p probes --release --example terrain_brush_resource_audit -- <SimCityData>
//!
//! This is deliberately read-only.  It does not infer that a menu item is
//! executable; it only proves whether the selector's brush keys resolve to
//! property resources and records their typed-map metadata.

use std::collections::{HashMap, HashSet};
use std::io::Cursor;
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
const TERRAIN_BRUSH_COMPONENT: u16 = 32;

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

fn read_be_u32(r: &mut Cursor<&[u8]>) -> Option<u32> {
    let mut b = [0; 4];
    std::io::Read::read_exact(r, &mut b).ok()?;
    Some(u32::from_be_bytes(b))
}

fn read_be_f32(r: &mut Cursor<&[u8]>) -> Option<f32> {
    Some(f32::from_bits(read_be_u32(r)?))
}

fn read_curve_len(r: &mut Cursor<&[u8]>) -> Option<usize> {
    let len = read_be_u32(r)? as usize;
    if len > 16_384 { return None; }
    for _ in 0..len { read_be_f32(r)?; }
    Some(len)
}

fn skip_be_floats(r: &mut Cursor<&[u8]>, count: usize) -> Option<()> {
    for _ in 0..count { read_be_f32(r)?; }
    Some(())
}

fn terrain_brush_descriptions(payload: &[u8], version: u16, count: u32) -> Vec<String> {
    let mut r = Cursor::new(payload);
    let mut out = Vec::new();
    for index in 0..count as usize {
        let start = r.position() as usize;
        // location, texture key, draw mode, gaussian slope and lifetime
        skip_be_floats(&mut r, 3);
        let mut texture = [0; 8];
        if std::io::Read::read_exact(&mut r, &mut texture).is_err() { break; }
        let mut b = [0; 1];
        if std::io::Read::read_exact(&mut r, &mut b).is_err() { break; }
        let draw_mode = b[0];
        let gaussian_slope = match read_be_f32(&mut r) { Some(v) => v, None => break };
        let lifetime = match read_be_f32(&mut r) { Some(v) => v, None => break };
        let intensity_keys = match read_curve_len(&mut r) { Some(v) => v, None => break };
        let size_keys = match read_curve_len(&mut r) { Some(v) => v, None => break };
        let rotation_keys = match read_curve_len(&mut r) { Some(v) => v, None => break };
        // intensity/size/rotation variation, level and level-range values
        if skip_be_floats(&mut r, 7 + 7).is_none() { break; }
        if version >= 3 && read_be_f32(&mut r).is_none() { break; }
        if skip_be_floats(&mut r, 4).is_none() { break; }
        let mut two = [0; 2];
        if std::io::Read::read_exact(&mut r, &mut two).is_err() { break; }
        let spacing_keys = match read_curve_len(&mut r) { Some(v) => v, None => break };
        if read_be_f32(&mut r).is_none() { break; }
        if std::io::Read::read_exact(&mut r, &mut b).is_err() { break; }
        if skip_be_floats(&mut r, 4).is_none() { break; }
        if std::io::Read::read_exact(&mut r, &mut b).is_err() { break; }
        if skip_be_floats(&mut r, 4).is_none() { break; }
        if std::io::Read::read_exact(&mut r, &mut b).is_err() { break; }
        let end = r.position() as usize;
        out.push(format!(
            "component=32#{index} version={version} bytes={}-{} texture={:02X?} draw_mode={draw_mode} gaussian_slope={gaussian_slope:.3} lifetime={lifetime:.3} curves=intensity:{intensity_keys},size:{size_keys},rotation:{rotation_keys},spacing:{spacing_keys}",
            start, end, texture
        ));
    }
    out
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
    let mut effect_ids = HashSet::new();
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
                effect_ids.insert(child.instance);
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

    // The child key is a Swarm exported-effect name hash, not a DBPF child
    // resource.  cTerrainScriptEffect passes only key.instance to
    // cEffectsWorld::CreateInternalEffect, which resolves the hash inside the
    // 0xEA5118B0 collection directory.  Report the real executable closure.
    let mut matched = 0usize;
    let mut terrain_components = 0usize;
    let mut collection_hits: HashMap<(u32, u32), usize> = HashMap::new();
    println!("\ntyped-map Swarm export closure:");
    for (path, package) in &files {
        for entry in package.entries() {
            if entry.id.type_id != 0xEA51_18B0 { continue; }
            let Ok(bytes) = package.read(entry) else { continue };
            let Ok(collection) = sc_properties::swarm::SwarmCollection::parse(&bytes) else { continue };
            if let Some(section) = collection.component_section(TERRAIN_BRUSH_COMPONENT) {
                println!("  component-section type=32 version={} count={} payload={} collection={:08X}:{:08X}", section.version, section.count, section.payload_size, entry.id.group, entry.id.instance);
                if let Some(end) = section.payload_offset.checked_add(section.payload_size) {
                    if let Some(payload) = bytes.get(section.payload_offset..end) {
                        let descriptions = terrain_brush_descriptions(payload, section.version, section.count);
                        println!("    decoded type=32 descriptions={}/{}", descriptions.len(), section.count);
                        for description in descriptions.iter().take(3) {
                            println!("    {description}");
                        }
                    }
                }
            }
            let terrain_descriptions = collection
                .component_section(TERRAIN_BRUSH_COMPONENT)
                .and_then(|section| section.payload_offset.checked_add(section.payload_size).and_then(|end| bytes.get(section.payload_offset..end).map(|payload| terrain_brush_descriptions(payload, section.version, section.count))))
                .unwrap_or_default();
            let meta_components = sc_properties::swarm_particle::decode_meta_particle_components(&collection, &bytes).ok();
            for export in &collection.exports {
                if !effect_ids.contains(&export.name_hash) { continue; }
                matched += 1;
                *collection_hits.entry((entry.id.group, entry.id.instance)).or_default() += 1;
                let Some(index) = export.effect_index else {
                    println!("  import {:08X} collection={:08X}:{:08X} source={}", export.name_hash, entry.id.group, entry.id.instance, path.display());
                    continue;
                };
                let Some(effect) = collection.visual_effects.get(index) else { continue };
                let blocks: Vec<String> = effect.blocks.iter().map(|block| {
                    if block.type_code == TERRAIN_BRUSH_COMPONENT as u8 { terrain_components += 1; }
                    if block.type_code == 2 {
                        if let Some(Some(meta)) = meta_components.as_ref().map(|items| items.get(block.component_index as usize)) {
                            if let Some(child) = &meta.child {
                                let mut detail = format!("{}#{}=>{}#{}", block.type_code, block.component_index, child.type_code, child.index);
                                if child.type_code == TERRAIN_BRUSH_COMPONENT {
                                    terrain_components += 1;
                                    if let Some(description) = terrain_descriptions.get(child.index) {
                                        detail.push_str(&format!(" [{description}]"));
                                    } else if let Some(section) = collection.component_section(TERRAIN_BRUSH_COMPONENT) {
                                        detail.push_str(&format!(" [section v{} count={} payload={}]", section.version, section.count, section.payload_size));
                                    }
                                }
                                return detail;
                            }
                        }
                    }
                    format!("{}#{}", block.type_code, block.component_index)
                }).collect();
                println!("  export {:08X} visual={} blocks=[{}] collection={:08X}:{:08X} source={}", export.name_hash, index, blocks.join(","), entry.id.group, entry.id.instance, path.display());
            }
        }
    }
    println!("swarm exports matched={matched} terrain_brush_blocks={terrain_components} collections={}", collection_hits.len());
    for ((group, instance), count) in collection_hits { println!("  collection={group:08X}:{instance:08X} exports={count}"); }
}
