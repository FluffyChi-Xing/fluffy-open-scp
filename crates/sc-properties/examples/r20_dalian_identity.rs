//! R20 isolation: keep the known-working Dalian identity/menu/property chain,
//! replace only its four LOD model payloads with the R17 tower model and its
//! native NightHouse material resources.

use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay_to_path};
use rw4::{Rw4File, SectionType};
use std::path::Path;

const R17_MODEL: ResourceId = ResourceId {
    type_id: 0x2F4E_681B,
    group: 0,
    instance: 0x5C78_6C64,
};
const DALIAN_MODELS: [ResourceId; 4] = [
    ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: 0xB9B0_8494,
    },
    ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: 0xB9B0_8495,
    },
    ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: 0xB9B0_8496,
    },
    ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: 0xB9B0_8497,
    },
];
fn dalian_material_model(r17: &[u8], dalian: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let source = Rw4File::parse(r17)?;
    let target = Rw4File::parse(dalian)?;
    let source_section = source
        .sections_of_type(SectionType::MATERIAL)
        .next()
        .ok_or("R17 material missing")?;
    let target_section = target
        .sections_of_type(SectionType::MATERIAL)
        .next()
        .ok_or("Dalian material missing")?;
    let source_payload = source.payload(r17, source_section.number)?;
    let target_payload = target.payload(dalian, target_section.number)?;
    if source_payload.len() != target_payload.len() {
        return Err("R17/Dalian material payload sizes differ".into());
    }
    let start = source_section.pos as usize;
    let mut out = r17.to_vec();
    out[start..start + target_payload.len()].copy_from_slice(target_payload);
    Ok(out)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dalian_path = std::env::args().nth(1).unwrap_or_else(|| r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SC5_DalianHopeTower_Education_0_8_5.package".into());
    let r17_path = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "tmp/nh_r11_menu_tower.package".into());
    let output = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "tmp/000_r20_dalian_identity.package".into());
    let dalian = Package::open(&dalian_path)?;
    let r17 = Package::open(&r17_path)?;
    let r17_model = r17.entry(R17_MODEL).ok_or("R17 model missing")?;
    let dalian_model = dalian
        .entry(DALIAN_MODELS[0])
        .ok_or("Dalian model missing")?;
    let model = dalian_material_model(&r17.read(r17_model)?, &dalian.read(dalian_model)?)?;
    let mut entries = Vec::with_capacity(dalian.entries().len());
    for entry in dalian.entries() {
        let bytes = if DALIAN_MODELS.contains(&entry.id) {
            model.clone()
        } else {
            dalian.read(entry)?
        };
        entries.push(OverlayEntry::new(entry.id, bytes));
    }
    let path = Path::new(&output);
    write_uncompressed_overlay_to_path(path, &entries)?;
    println!(
        "R20: {} entries -> {} (Dalian identity + R17 model)",
        entries.len(),
        path.display()
    );
    Ok(())
}
