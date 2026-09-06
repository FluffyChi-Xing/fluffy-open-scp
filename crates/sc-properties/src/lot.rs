use serde::Serialize;

use crate::{Key, Kind, PropertyFile, Transform, Value};

pub const PROPERTY_RESOURCE_TYPE: u32 = 0x00B1_B104;
pub const LOD1_MODEL_HASH: u32 = 0x00F9_EFBB;
pub const LOT_MASK_HASH: u32 = 0x0CCB_7FD5;
pub const LOT_SIZE_HASH: u32 = 0x0CCB_7FC8;
pub const LOT_PLACEMENT_HASH: u32 = 0x0DB7_FB17;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LotEditorDocument {
    pub properties: PropertyFile,
    pub model: Option<Key>,
    pub lot_size: Option<[f32; 2]>,
    pub placement: Option<Transform>,
    pub lot_mask: Option<Key>,
    pub unknown_property_count: usize,
}

impl LotEditorDocument {
    pub fn from_property_file(properties: PropertyFile) -> Self {
        let model = scalar_key(&properties, LOD1_MODEL_HASH);
        let lot_mask = scalar_key(&properties, LOT_MASK_HASH);
        let lot_size = value_vec2(&properties, LOT_SIZE_HASH);
        let placement = value_transform(&properties, LOT_PLACEMENT_HASH);
        let known = [
            LOD1_MODEL_HASH,
            LOT_MASK_HASH,
            LOT_SIZE_HASH,
            LOT_PLACEMENT_HASH,
        ];
        let unknown_property_count = properties
            .values
            .iter()
            .filter(|property| !known.contains(&property.hash))
            .count();
        Self {
            properties,
            model,
            lot_size,
            placement,
            lot_mask,
            unknown_property_count,
        }
    }

    pub fn into_property_file(self) -> PropertyFile {
        self.properties
    }

    /// 把文档属性字典装配为强类型 Unit 列表（见 [`crate::lot_unit`]）。
    pub fn assemble_units(&self) -> crate::lot_unit::LotUnits {
        crate::lot_unit::assemble_units(&self.properties)
    }

    pub fn clone_deep(&self) -> Self {
        self.clone()
    }
}

fn scalar_key(properties: &PropertyFile, hash: u32) -> Option<Key> {
    match &properties.get(hash)?.kind {
        Kind::Scalar(Value::Key(key)) => Some(*key),
        _ => None,
    }
}

fn value_vec2(properties: &PropertyFile, hash: u32) -> Option<[f32; 2]> {
    match &properties.get(hash)?.kind {
        Kind::Scalar(Value::Vector2(value)) => Some(*value),
        _ => None,
    }
}

fn value_transform(properties: &PropertyFile, hash: u32) -> Option<Transform> {
    match &properties.get(hash)?.kind {
        Kind::Scalar(Value::Transform(value)) => Some(value.clone()),
        _ => None,
    }
}
