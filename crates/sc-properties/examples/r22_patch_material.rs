use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay_to_path};
use rw4::{Rw4File, SectionType};
use std::path::Path;

const MODEL: ResourceId = ResourceId {
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

fn patch_slots(
    mut model: Vec<u8>,
    replacements: [u32; 6],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let file = Rw4File::parse(&model)?;
    let section = file
        .sections_of_type(SectionType::MATERIAL)
        .next()
        .ok_or("material missing")?;
    let payload = file.payload(&model, section.number)?.to_vec();
    let decoded = file.decode_material(&model, section.number)?;
    let rw4::MaterialSection::Decoded(material) = decoded else {
        return Err("unknown material layout".into());
    };
    let mut out = model;
    let start = section.pos as usize;
    for (slot, replacement) in material.texture_slots().zip(replacements) {
        let old = slot.texture_instance.to_le_bytes();
        let new = replacement.to_le_bytes();
        let mut hits = 0;
        for off in 0..payload.len().saturating_sub(3) {
            if payload[off..off + 4] == old {
                out[start + off..start + off + 4].copy_from_slice(&new);
                hits += 1;
            }
        }
        if hits == 0 {
            return Err(format!(
                "slot{} instance {:08X} not found",
                slot.slot_byte(),
                slot.texture_instance
            )
            .into());
        }
    }
    Ok(out)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tmp/000_r20_dalian_identity.package".into());
    let source = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "tmp/nh_r11_menu_tower.package".into());
    let textures = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "tmp/000_r21_copan_textured.package".into());
    let output = std::env::args()
        .nth(4)
        .unwrap_or_else(|| "tmp/000_r22_copan_textured_safe.package".into());
    let base_pkg = Package::open(&base)?;
    let src_pkg = Package::open(&source)?;
    let tex_pkg = Package::open(&textures)?;
    let src_entry = src_pkg.entry(MODEL).ok_or("R17 model missing")?;
    let mut model = src_pkg.read(src_entry)?;
    let sf = Rw4File::parse(&model)?;
    let sec = sf
        .sections_of_type(SectionType::MATERIAL)
        .next()
        .ok_or("material missing")?;
    let mat = sf.decode_material(&model, sec.number)?;
    let rw4::MaterialSection::Decoded(mat) = mat else {
        return Err("unknown material".into());
    };
    let slots: Vec<u32> = mat.texture_slots().map(|s| s.texture_instance).collect();
    println!(
        "old slots: {:?}",
        slots
            .iter()
            .map(|v| format!("{:08X}", v))
            .collect::<Vec<_>>()
    );
    let ids: [ResourceId; 6] = [
        ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance: 0xF9CC_7D44,
        },
        ResourceId {
            type_id: 0x2F4E_681C,
            group: 0,
            instance: 0x3A8D_99BE,
        },
        ResourceId {
            type_id: 0x2F4E_681C,
            group: 0,
            instance: 0x3BEB_26BE,
        },
        ResourceId {
            type_id: 0x2F4E_681C,
            group: 0,
            instance: 0x9935_86E0,
        },
        ResourceId {
            type_id: 0x2F4E_681C,
            group: 0,
            instance: 0xA312_DF04,
        },
        ResourceId {
            type_id: 0x2F4E_681C,
            group: 0,
            instance: 0x3363_5ED5,
        },
    ];
    model = patch_slots(model, ids.map(|id| id.instance))?;
    let mut entries = Vec::new();
    for e in base_pkg.entries() {
        let bytes = if DALIAN_MODELS.contains(&e.id) {
            model.clone()
        } else {
            base_pkg.read(e)?
        };
        entries.push(OverlayEntry::new(e.id, bytes));
    }
    for id in ids {
        let e = tex_pkg
            .entry(id)
            .ok_or_else(|| format!("missing texture {id}"))?;
        entries.push(OverlayEntry::new(id, tex_pkg.read(e)?));
    }
    write_uncompressed_overlay_to_path(Path::new(&output), &entries)?;
    println!("R22 wrote {} entries to {}", entries.len(), output);
    Ok(())
}
