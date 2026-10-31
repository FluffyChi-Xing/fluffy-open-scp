//! R15 = R14 with the model's native six-slot material closure restored.
//!
//! The R14 model was structurally valid but its MATERIAL section referenced
//! NightHouse test texture instances that were not present in the package.
//! R15 keeps the R14 geometry, replaces only the MATERIAL payload with the
//! verified Dalian Building40 material, and carries the six matching raster
//! resources into the output package.

use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay_to_path};
use rw4::{Rw4File, SectionType};
use std::path::Path;

const MODEL: ResourceId = ResourceId {
    type_id: 0x2F4E_681B,
    group: 0,
    instance: 0x8A09_6154,
};
const DALIAN_MATERIAL_RESOURCES: [ResourceId; 6] = [
    ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: 0x921D_4B66,
    }, // slot0 paletteF32
    ResourceId {
        type_id: 0x2F4E_681C,
        group: 0,
        instance: 0x921D_4B67,
    }, // slot1 tint
    ResourceId {
        type_id: 0x2F4E_681C,
        group: 0,
        instance: 0x921D_4B64,
    }, // slot2 normal
    ResourceId {
        type_id: 0x2F4E_681C,
        group: 0,
        instance: 0x921D_4B65,
    }, // slot3 shader
    ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: 0x921D_4B62,
    }, // slot4 palette
    ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: 0x921D_4B63,
    }, // slot5 interior
];

fn patch_material(model: &[u8], verified: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let target = Rw4File::parse(model)?;
    let source = Rw4File::parse(verified)?;
    let target_section = target
        .sections_of_type(SectionType::MATERIAL)
        .next()
        .ok_or("R14 model has no MATERIAL section")?;
    let source_section = source
        .sections_of_type(SectionType::MATERIAL)
        .next()
        .ok_or("Dalian model has no MATERIAL section")?;
    let target_payload = target.payload(model, target_section.number)?;
    let source_payload = source.payload(verified, source_section.number)?;
    if target_payload.len() != source_payload.len() {
        return Err(format!(
            "material payload size differs: R14={} Dalian={}",
            target_payload.len(),
            source_payload.len()
        )
        .into());
    }
    let start = target_section.pos as usize;
    let end = start + source_payload.len();
    let mut patched = model.to_vec();
    patched[start..end].copy_from_slice(source_payload);
    // The patched material must decode to exactly six non-zero references.
    let check = Rw4File::parse(&patched)?;
    let section = check
        .sections_of_type(SectionType::MATERIAL)
        .next()
        .unwrap();
    let decoded = check.decode_material(&patched, section.number)?;
    let slots = decoded.texture_refs();
    if slots.len() < 7 || slots.iter().skip(1).any(|slot| slot.texture_instance == 0) {
        return Err("patched material does not contain six populated texture slots".into());
    }
    Ok(patched)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let r14_path = std::env::args().nth(1).unwrap_or_else(|| {
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\000_r14_notex.package".into()
    });
    let dalian_path = std::env::args().nth(2).unwrap_or_else(|| {
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SC5_DalianHopeTower_Education_0_8_5.package".into()
    });
    let out_path = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "tmp/000_r15.package".into());

    let r14 = Package::open(&r14_path)?;
    let dalian = Package::open(&dalian_path)?;
    let r14_model = r14.entry(MODEL).ok_or("R14 model 8A096154 is missing")?;
    let dalian_model = dalian
        .entry(ResourceId {
            type_id: MODEL.type_id,
            group: 0,
            instance: 0xB9B0_8494,
        })
        .ok_or("Dalian Building40 model B9B08494 is missing")?;
    let patched_model = patch_material(&r14.read(r14_model)?, &dalian.read(dalian_model)?)?;

    let mut entries = Vec::with_capacity(r14.entries().len() + DALIAN_MATERIAL_RESOURCES.len());
    for entry in r14.entries() {
        let bytes = if entry.id == MODEL {
            patched_model.clone()
        } else {
            r14.read(entry)?
        };
        entries.push(OverlayEntry::new(entry.id, bytes));
    }
    for id in DALIAN_MATERIAL_RESOURCES {
        let entry = dalian
            .entry(id)
            .ok_or_else(|| format!("Dalian material resource {id} is missing"))?;
        entries.push(OverlayEntry::new(id, dalian.read(entry)?));
    }
    let output = Path::new(&out_path);
    write_uncompressed_overlay_to_path(output, &entries)?;
    let written = Package::open(output)?;
    for id in DALIAN_MATERIAL_RESOURCES {
        if written.entry(id).is_none() {
            return Err(format!("R15 readback missing {id}").into());
        }
    }
    println!("R15: {} entries -> {}", entries.len(), output.display());
    println!("model {} now uses Dalian six-slot texture TGIs", MODEL);
    Ok(())
}
