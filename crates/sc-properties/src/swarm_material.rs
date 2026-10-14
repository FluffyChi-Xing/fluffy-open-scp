//! Texture references stored in Swarm material auxiliary resources.

use std::collections::HashMap;

use crate::swarm::{SwarmCollection, SwarmError};

const MATERIAL_AUX_TYPE: u16 = 1;
const MAX_PARAMS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialTexture {
    pub group: u32,
    pub instance: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectMaterial {
    pub group: u32,
    pub instance: u32,
    pub shader_group: u32,
    pub shader_instance: u32,
    pub format: u32,
    pub textures: Vec<(u32, MaterialTexture)>,
}

pub fn decode_effect_materials(
    collection: &SwarmCollection,
    data: &[u8],
) -> Result<HashMap<(u32, u32), EffectMaterial>, SwarmError> {
    let mut materials = HashMap::new();
    for section in collection
        .resources
        .iter()
        .filter(|s| s.type_code == MATERIAL_AUX_TYPE)
    {
        if section.version > 1 {
            return Err(SwarmError::Invalid(
                section.payload_offset,
                "unsupported material version",
            ));
        }
        let end = section
            .payload_offset
            .checked_add(section.payload_size)
            .ok_or(SwarmError::Truncated(section.payload_offset))?;
        let payload = data
            .get(section.payload_offset..end)
            .ok_or(SwarmError::Truncated(section.payload_offset))?;
        let mut reader = Reader {
            data: payload,
            pos: 0,
            base: section.payload_offset,
        };
        for _ in 0..section.count {
            let group = reader.u32()?;
            let instance = reader.u32()?;
            let shader_group = reader.u32()?;
            let shader_instance = reader.u32()?;
            let format = if section.version > 0 {
                reader.u32()?
            } else {
                33
            };
            let count = reader.u32()? as usize;
            if count > MAX_PARAMS {
                return Err(SwarmError::Invalid(
                    reader.base + reader.pos - 4,
                    "too many material parameters",
                ));
            }
            let mut textures = Vec::new();
            for _ in 0..count {
                let parameter_group = reader.u32()?;
                let parameter_id = reader.u32()?;
                let kind = reader.u8()?;
                match kind {
                    0 | 1 => reader.skip(4)?,
                    2 => reader.skip(1)?,
                    3 | 4 | 5 => {
                        let values = reader.u16()? as usize;
                        reader.skip(values * if kind == 5 { 1 } else { 4 })?;
                    }
                    6 => {
                        let texture_group = reader.u32()?;
                        let texture_instance = reader.u32()?;
                        if parameter_group == 0 && parameter_id <= 7 {
                            textures.push((
                                parameter_id,
                                MaterialTexture {
                                    group: texture_group,
                                    instance: texture_instance,
                                },
                            ));
                        }
                    }
                    _ => {
                        return Err(SwarmError::Invalid(
                            reader.base + reader.pos - 1,
                            "unknown material parameter type",
                        ));
                    }
                }
            }
            materials.insert(
                (group, instance),
                EffectMaterial {
                    group,
                    instance,
                    shader_group,
                    shader_instance,
                    format,
                    textures,
                },
            );
        }
        if reader.pos != payload.len() {
            return Err(SwarmError::Invalid(
                reader.base + reader.pos,
                "material section size mismatch",
            ));
        }
    }
    Ok(materials)
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    base: usize,
}

impl Reader<'_> {
    fn take(&mut self, len: usize) -> Result<&[u8], SwarmError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(SwarmError::Truncated(self.base + self.pos))?;
        let result = self
            .data
            .get(self.pos..end)
            .ok_or(SwarmError::Truncated(self.base + self.pos))?;
        self.pos = end;
        Ok(result)
    }
    fn skip(&mut self, len: usize) -> Result<(), SwarmError> {
        self.take(len).map(|_| ())
    }
    fn u8(&mut self) -> Result<u8, SwarmError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, SwarmError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, SwarmError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swarm::Section;

    #[test]
    fn decodes_texture_parameter_and_skips_scalar() {
        let mut bytes = Vec::new();
        for value in [0x4028_0000u32, 0x1234_5678, 0, 0x1122_3344, 33, 2, 0, 8] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        bytes.push(0);
        bytes.extend_from_slice(&1f32.to_bits().to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.push(6);
        bytes.extend_from_slice(&0x4028_0000u32.to_be_bytes());
        bytes.extend_from_slice(&0xAABB_CCDDu32.to_be_bytes());
        let collection = SwarmCollection {
            components: vec![],
            resources: vec![Section {
                type_code: 1,
                version: 1,
                count: 1,
                payload_offset: 0,
                payload_size: bytes.len(),
            }],
            visual_version: 0,
            visual_effects: vec![],
            exports: vec![],
            imports: vec![],
        };
        let materials = decode_effect_materials(&collection, &bytes).unwrap();
        assert_eq!(
            materials[&(0x4028_0000, 0x1234_5678)].textures[0],
            (
                0,
                MaterialTexture {
                    group: 0x4028_0000,
                    instance: 0xAABB_CCDD
                }
            )
        );
    }

    #[test]
    fn retail_material_sections_align() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tmp/gltf_fixture/effects_main.col"
        );
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let collection = SwarmCollection::parse(&bytes).unwrap();
        let materials = decode_effect_materials(&collection, &bytes).unwrap();
        assert!(!materials.is_empty());
        assert!(
            materials
                .values()
                .any(|material| !material.textures.is_empty())
        );
    }

    #[test]
    fn ep1_material_references_have_texture_slots_when_installed() {
        use crate::swarm::EFFECT_COLLECTION_TYPE;
        let path = "D:/ea-games/SimCity/SimCityData/SimCity_App.package";
        let Ok(package) = dbpf::Package::open(path) else {
            return;
        };
        let graphics =
            dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_Graphics.package")
                .unwrap();
        let entry = package
            .entries()
            .iter()
            .find(|entry| {
                entry.id.type_id == EFFECT_COLLECTION_TYPE && entry.id.instance == 0x622B_9CD7
            })
            .expect("EP1 effect collection");
        let bytes = package.read(entry).unwrap();
        let collection = SwarmCollection::parse(&bytes).unwrap();
        let materials = decode_effect_materials(&collection, &bytes).unwrap();
        let particles =
            crate::swarm_particle::decode_particle_components(&collection, &bytes).unwrap();
        let material_particles: Vec<_> = particles
            .iter()
            .filter(|particle| particle.texture.draw_mode == 15)
            .collect();
        let linked = material_particles
            .iter()
            .filter(|particle| {
                materials
                    .get(&(particle.texture.group, particle.texture.instance))
                    .is_some_and(|material| !material.textures.is_empty())
            })
            .count();
        assert_eq!(linked, material_particles.len());
        for (material_id, texture_id, tiles) in [
            (0x684A_2E36, 0xC0D1_89FE, [1, 16]),
            (0x93DC_33AD, 0xA148_458F, [1, 1]),
            (0xFA57_D08F, 0xD7E5_6466, [1, 16]),
        ] {
            let material = materials.get(&(0, material_id)).unwrap();
            assert_eq!(
                material.textures[0],
                (
                    2,
                    MaterialTexture {
                        group: 0,
                        instance: texture_id
                    }
                )
            );
            assert!(particles.iter().any(|particle| {
                particle.texture.instance == material_id && particle.tile_count == tiles
            }));
            let entry = graphics
                .entries()
                .iter()
                .find(|entry| {
                    entry.id.type_id == 0x2F4E_681B
                        && entry.id.group == 0
                        && entry.id.instance == texture_id
                })
                .unwrap();
            let bytes = graphics.read(entry).unwrap();
            let file = rw4::Rw4File::parse(&bytes).unwrap();
            let section = file
                .sections_of_type(rw4::SectionType::TEXTURE)
                .next()
                .unwrap();
            let decoded = file.decode_texture(&bytes, section.number).unwrap();
            let rgba = decoded.decode_top_mip_rgba().unwrap();
            let image =
                image::RgbaImage::from_raw(decoded.width.into(), decoded.height.into(), rgba)
                    .unwrap();
            let mut png = std::io::Cursor::new(Vec::new());
            image.write_to(&mut png, image::ImageFormat::Png).unwrap();
            assert!(png.get_ref().len() <= 512 * 1024);
        }
    }
}
