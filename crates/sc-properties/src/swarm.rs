//! Structural decoder for EA Swarm effect collections (`0xEA5118B0`).
//!
//! Component/resource payloads are preserved as bounded sections. Visual effect
//! blocks and the exported name hashes are decoded so a lot effect key can be
//! resolved without loading particle data or constructing a renderer.

use serde::Serialize;
use thiserror::Error;

pub const EFFECT_COLLECTION_TYPE: u32 = 0xEA51_18B0;
const IMPORT_MASK: u32 = 0x7F00_0000;
const MAX_ITEMS: usize = 1_000_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SwarmError {
    #[error("truncated Swarm collection at offset {0}")]
    Truncated(usize),
    #[error("unsupported Swarm collection version {0}")]
    UnsupportedVersion(u16),
    #[error("invalid Swarm collection at offset {0}: {1}")]
    Invalid(usize, &'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub type_code: u16,
    pub version: u16,
    pub count: u32,
    pub payload_offset: usize,
    pub payload_size: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VisualBlock {
    pub type_code: u8,
    pub flags: u32,
    pub transform: BlockTransform,
    pub component_index: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BlockTransform {
    pub scale: f32,
    pub rotation: [f32; 9],
    pub offset: [f32; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct VisualEffect {
    pub flags: u32,
    pub blocks: Vec<VisualBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub name_hash: u32,
    pub group_id: u32,
    /// Index in `visual_effects`; `None` identifies an exported import.
    pub effect_index: Option<usize>,
    pub import_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub group_id: u32,
    pub name_hash: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwarmCollection {
    pub components: Vec<Section>,
    pub resources: Vec<Section>,
    pub visual_version: u16,
    pub visual_effects: Vec<VisualEffect>,
    pub exports: Vec<Export>,
    pub imports: Vec<Import>,
}

impl SwarmCollection {
    pub fn parse(data: &[u8]) -> Result<Self, SwarmError> {
        let mut r = Reader { data, pos: 0 };
        let version = r.u16()?;
        if version != 4 {
            return Err(SwarmError::UnsupportedVersion(version));
        }
        let components = read_sections(&mut r)?;
        let resources = read_sections(&mut r)?;
        let visual_version = r.u16()?;
        let visual_count = r.count()?;
        let mut visual_effects = Vec::with_capacity(visual_count);
        for _ in 0..visual_count {
            visual_effects.push(read_visual_effect(&mut r)?);
        }

        let mut exports = Vec::new();
        loop {
            let at = r.pos;
            let index = r.u32()?;
            if index == u32::MAX {
                break;
            }
            let group_id = r.u32()?;
            let name_hash = r.u32()?;
            let (effect_index, import_index) = if index & IMPORT_MASK == IMPORT_MASK {
                (None, Some((index & !IMPORT_MASK) as usize))
            } else {
                let index = index as usize;
                if index >= visual_effects.len() {
                    return Err(SwarmError::Invalid(
                        at,
                        "exported effect index out of range",
                    ));
                }
                (Some(index), None)
            };
            if exports.len() >= MAX_ITEMS {
                return Err(SwarmError::Invalid(at, "too many exports"));
            }
            exports.push(Export {
                name_hash,
                group_id,
                effect_index,
                import_index,
            });
        }

        r.u32()?; // String-assignment field, unused by the effect directory.
        let import_count = r.count()?;
        let mut imports = Vec::with_capacity(import_count);
        for _ in 0..import_count {
            imports.push(Import {
                group_id: r.u32()?,
                name_hash: r.u32()?,
            });
        }
        if exports
            .iter()
            .any(|export| export.import_index.is_some_and(|i| i >= imports.len()))
        {
            return Err(SwarmError::Invalid(
                r.pos,
                "exported import index out of range",
            ));
        }
        if r.pos != data.len() {
            return Err(SwarmError::Invalid(r.pos, "trailing collection data"));
        }
        Ok(Self {
            components,
            resources,
            visual_version,
            visual_effects,
            exports,
            imports,
        })
    }

    pub fn exported_effect(&self, name_hash: u32) -> Option<&VisualEffect> {
        self.exports
            .iter()
            .find(|entry| entry.name_hash == name_hash)
            .and_then(|entry| entry.effect_index)
            .and_then(|index| self.visual_effects.get(index))
    }

    pub fn component_section(&self, type_code: u16) -> Option<&Section> {
        self.components
            .iter()
            .find(|section| section.type_code == type_code)
    }
}

fn read_sections(r: &mut Reader<'_>) -> Result<Vec<Section>, SwarmError> {
    let mut sections = Vec::new();
    loop {
        let at = r.pos;
        let type_code = r.u16()?;
        if type_code == u16::MAX {
            break;
        }
        let version = r.u16()?;
        let size = r.u32()? as usize;
        let count = r.u32()?;
        if size < 4 || count as usize > MAX_ITEMS {
            return Err(SwarmError::Invalid(at, "invalid section size or count"));
        }
        let payload_offset = r.pos;
        let payload_size = size - 4;
        r.skip(payload_size)?;
        if sections.len() >= 64 {
            return Err(SwarmError::Invalid(at, "too many sections"));
        }
        sections.push(Section {
            type_code,
            version,
            count,
            payload_offset,
            payload_size,
        });
    }
    Ok(sections)
}

fn read_visual_effect(r: &mut Reader<'_>) -> Result<VisualEffect, SwarmError> {
    let flags = r.u32()?;
    r.skip(8 + 8 + 4 + 1)?; // app flags, notify ID, screen range, cursor distance/button
    let lod_count = r.count()?;
    r.skip_array(lod_count, 4)?;
    r.skip(12 + 4)?; // extended LOD weights and seed
    let block_count = r.count()?;
    let mut blocks = Vec::with_capacity(block_count);
    for _ in 0..block_count {
        let type_code = r.u8()?;
        let flags = r.u32()?;
        r.skip(2)?; // transform flags
        let scale = f32::from_bits(r.u32()?);
        let mut rotation = [0.0; 9];
        for value in &mut rotation {
            *value = f32::from_bits(r.u32_le()?);
        }
        let mut offset = [0.0; 3];
        for value in &mut offset {
            *value = f32::from_bits(r.u32_le()?);
        }
        r.skip(2)?; // LOD endpoints
        let scale_count = r.count()?;
        r.skip_array(scale_count, 12)?;
        r.skip(24 + 8 + 2 + 2 + 4)?; // scales, app flags, selection, time scale
        let component_index = r.u32()? as i32;
        blocks.push(VisualBlock {
            type_code,
            flags,
            transform: BlockTransform {
                scale,
                rotation,
                offset,
            },
            component_index,
        });
    }
    Ok(VisualEffect { flags, blocks })
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, len: usize) -> Result<&[u8], SwarmError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(SwarmError::Truncated(self.pos))?;
        let bytes = self
            .data
            .get(self.pos..end)
            .ok_or(SwarmError::Truncated(self.pos))?;
        self.pos = end;
        Ok(bytes)
    }
    fn skip(&mut self, len: usize) -> Result<(), SwarmError> {
        self.take(len).map(|_| ())
    }
    fn skip_array(&mut self, count: usize, stride: usize) -> Result<(), SwarmError> {
        self.skip(
            count
                .checked_mul(stride)
                .ok_or(SwarmError::Truncated(self.pos))?,
        )
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
    fn u32_le(&mut self) -> Result<u32, SwarmError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn count(&mut self) -> Result<usize, SwarmError> {
        let at = self.pos;
        let count = self.u32()? as usize;
        if count > MAX_ITEMS {
            return Err(SwarmError::Invalid(at, "item count exceeds limit"));
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_empty_collection() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&4u16.to_be_bytes());
        bytes.extend_from_slice(&u16::MAX.to_be_bytes());
        bytes.extend_from_slice(&u16::MAX.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&u32::MAX.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        let collection = SwarmCollection::parse(&bytes).unwrap();
        assert!(collection.visual_effects.is_empty());
        assert!(collection.exports.is_empty());
        assert_eq!(
            SwarmCollection::parse(&bytes[..bytes.len() - 1]),
            Err(SwarmError::Truncated(bytes.len() - 4))
        );
    }

    #[test]
    fn resolves_exported_visual_blocks_and_rejects_bad_index() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&4u16.to_be_bytes());
        bytes.extend_from_slice(&u16::MAX.to_be_bytes());
        bytes.extend_from_slice(&u16::MAX.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&7u32.to_be_bytes());
        bytes.extend_from_slice(&[0; 21]);
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&[0; 16]);
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&9u32.to_be_bytes());
        bytes.extend_from_slice(&[0; 56]);
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&[0; 40]);
        bytes.extend_from_slice(&3u32.to_be_bytes());
        let export_offset = bytes.len();
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&0x1234u32.to_be_bytes());
        bytes.extend_from_slice(&u32::MAX.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());

        let collection = SwarmCollection::parse(&bytes).unwrap();
        let visual = collection.exported_effect(0x1234).unwrap();
        assert_eq!(visual.flags, 7);
        assert_eq!(
            visual.blocks,
            vec![VisualBlock {
                type_code: 1,
                flags: 9,
                transform: BlockTransform {
                    scale: 0.0,
                    rotation: [0.0; 9],
                    offset: [0.0; 3],
                },
                component_index: 3
            }]
        );
        assert!(collection.exported_effect(0x5678).is_none());

        bytes[export_offset..export_offset + 4].copy_from_slice(&1u32.to_be_bytes());
        assert_eq!(
            SwarmCollection::parse(&bytes),
            Err(SwarmError::Invalid(
                export_offset,
                "exported effect index out of range"
            ))
        );
    }

    #[test]
    fn parses_retail_fixture_when_available() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tmp/gltf_fixture/effects_main.col"
        );
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let collection = SwarmCollection::parse(&bytes).unwrap();
        assert_eq!(collection.visual_effects.len(), 347);
        assert!(!collection.exports.is_empty());
        assert!(
            collection
                .exports
                .iter()
                .all(|entry| entry.effect_index.is_some())
        );
    }
}
