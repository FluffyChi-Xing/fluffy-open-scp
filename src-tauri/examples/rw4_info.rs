use rw4::{Rw4File, SectionType};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or("usage: rw4_info <model.rw4>")?;
    let bytes = std::fs::read(path)?;
    let file = Rw4File::parse(&bytes)?;
    let count = |kind| file.sections_of_type(kind).count();
    println!(
        "mesh={} material={} mma={} texture={}",
        count(SectionType::MESH),
        count(SectionType::MATERIAL),
        count(SectionType::MESH_MATERIAL_ASSIGNMENT),
        count(SectionType::TEXTURE)
    );
    for section in file.sections_of_type(SectionType::MATERIAL) {
        match file.decode_material(&bytes, section.number)? {
            rw4::MaterialSection::Decoded(material) => {
                println!(
                    "material #{} header={:02x?} refs={:?}",
                    section.number,
                    material.header,
                    material
                        .texture_refs
                        .iter()
                        .map(|r| (r.slot_byte(), r.texture_instance))
                        .collect::<Vec<_>>()
                );
            }
            rw4::MaterialSection::Raw(payload) => {
                println!("material #{} raw {}B", section.number, payload.len());
            }
        }
    }
    Ok(())
}
