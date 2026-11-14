//! R18 isolation package: keep R17 identity/properties, inject the known-good
//! Dalian model payload under the existing R17 model TGI.

use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay_to_path};
use std::path::Path;

const TARGET_MODEL: ResourceId = ResourceId {
    type_id: 0x2F4E_681B,
    group: 0,
    instance: 0x5C78_6C64,
};
const DALIAN_MODEL: ResourceId = ResourceId {
    type_id: 0x2F4E_681B,
    group: 0,
    instance: 0xB9B0_8494,
};
const MATERIAL_RESOURCES: [ResourceId; 6] = [
    ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: 0x921D_4B66,
    },
    ResourceId {
        type_id: 0x2F4E_681C,
        group: 0,
        instance: 0x921D_4B67,
    },
    ResourceId {
        type_id: 0x2F4E_681C,
        group: 0,
        instance: 0x921D_4B64,
    },
    ResourceId {
        type_id: 0x2F4E_681C,
        group: 0,
        instance: 0x921D_4B65,
    },
    ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: 0x921D_4B62,
    },
    ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: 0x921D_4B63,
    },
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tmp/nh_r11_menu_tower.package".into());
    let dalian_path = std::env::args().nth(2).unwrap_or_else(|| r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SC5_DalianHopeTower_Education_0_8_5.package".into());
    let output = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "tmp/000_r18_known_model.package".into());
    let source = Package::open(&input)?;
    let dalian = Package::open(&dalian_path)?;
    let source_model = source
        .entry(TARGET_MODEL)
        .ok_or("R17 model TGI is missing")?;
    let dalian_model = dalian
        .entry(DALIAN_MODEL)
        .ok_or("Dalian model is missing")?;
    let known_model = dalian.read(dalian_model)?;
    let mut entries = Vec::with_capacity(source.entries().len() + MATERIAL_RESOURCES.len());
    for entry in source.entries() {
        let bytes = if entry.id == TARGET_MODEL {
            known_model.clone()
        } else {
            source.read(entry)?
        };
        entries.push(OverlayEntry::new(entry.id, bytes));
    }
    for id in MATERIAL_RESOURCES {
        let entry = dalian
            .entry(id)
            .ok_or_else(|| format!("Dalian material resource {id} is missing"))?;
        entries.push(OverlayEntry::new(id, dalian.read(entry)?));
    }
    let path = Path::new(&output);
    write_uncompressed_overlay_to_path(path, &entries)?;
    println!(
        "R18: {} entries -> {} (R17 identity + Dalian known-good model)",
        entries.len(),
        path.display()
    );
    Ok(())
}
