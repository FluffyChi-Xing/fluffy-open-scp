//! R16 isolation package: keep R15 resources/model, clear all Lot unit columns.
//! This isolates the save-load crash from prop/light/path/spawner references.

use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay_to_path};
use sc_properties::{PropertyFile, lot_unit as u};
use std::path::Path;

const LOT: ResourceId = ResourceId {
    type_id: 0x00B1_B104,
    group: 0x40E1_C000,
    instance: 0xBA52_24C6,
};

fn unit_hashes() -> Vec<u32> {
    let mut hashes = vec![
        u::LIGHT_IDS,
        u::LIGHT_TYPES,
        u::LIGHT_TRANSFORMS,
        u::LIGHT_COLORS,
        u::LIGHT_RADII,
        u::LIGHT_INNER_RADII,
        u::LIGHT_DIFFUSE_LEVELS,
        u::LIGHT_DEBUG_NAMES,
        u::LIGHT_SPEC_LEVELS,
        u::LIGHT_LENGTHS,
        u::LIGHT_CULL_DISTANCES,
        u::LIGHT_FALLOFF_STARTS,
        u::LIGHT_IS_VOLUMETRIC,
        u::LIGHT_VOL_STRENGTHS,
        u::EFFECT_IDS,
        u::EFFECT_TRANSFORMS,
        u::EFFECT_ALWAYS_ZERO,
        u::EFFECT_REF_IDS,
        u::EFFECT_ENABLED,
        u::PATH_POINT_POINTS,
        u::PATH_POINT_TANGENTS,
        u::PATH_POINT_INDICES,
        u::PATH_PAIRS,
        u::SPAWNER_IDS,
        u::SPAWNER_TRANSFORMS,
        u::SPAWNER_COUNT,
        u::SPAWNER_COUNT_RANDOM,
        u::SPAWNER_AGENT,
    ];
    for bin in 0..u::PROP_BINS as u32 {
        hashes.extend([
            u::PROP_ID_BASE + bin,
            u::PROP_TRANSFORM_BASE + bin,
            u::PROP_SLOT_BASE + bin,
            u::PROP_RANDOMIZE_SLOT_BASE + bin,
            u::PROP_PERCENT_FILL_BASE + bin,
        ]);
    }
    for category in 0..u::DECAL_CATEGORIES as u32 {
        hashes.extend([
            u::DECAL_ID_BASE + category,
            u::DECAL_TRANSFORM_BASE + category,
            u::DECAL_DEPTH_BASE + category,
            u::DECAL_MATERIAL_BASE + category,
        ]);
    }
    hashes
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tmp/000_r15.package".into());
    let output = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "tmp/000_r16_no_units.package".into());
    let package = Package::open(&input)?;
    let lot_entry = package.entry(LOT).ok_or("R15 Lot property is missing")?;
    let mut lot = PropertyFile::parse(&package.read(lot_entry)?)?;
    let hashes = unit_hashes();
    lot.values
        .retain(|property| !hashes.contains(&property.hash));
    let lot_bytes = lot.encode_canonical()?;

    let mut entries = Vec::with_capacity(package.entries().len());
    for entry in package.entries() {
        let bytes = if entry.id == LOT {
            lot_bytes.clone()
        } else {
            package.read(entry)?
        };
        entries.push(OverlayEntry::new(entry.id, bytes));
    }
    let path = Path::new(&output);
    write_uncompressed_overlay_to_path(path, &entries)?;
    let readback = Package::open(path)?;
    let check = PropertyFile::parse(&readback.read(readback.entry(LOT).unwrap())?)?;
    let remaining = check
        .values
        .iter()
        .filter(|p| hashes.contains(&p.hash))
        .count();
    if remaining != 0 {
        return Err(format!("R16 retained {remaining} unit fields").into());
    }
    println!(
        "R16: {} entries -> {} (all Lot unit columns cleared)",
        entries.len(),
        path.display()
    );
    Ok(())
}
