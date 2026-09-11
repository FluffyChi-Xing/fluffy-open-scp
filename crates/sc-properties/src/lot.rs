use serde::Serialize;

use crate::{Key, Kind, PropertyFile, Transform, Value};

pub const PROPERTY_RESOURCE_TYPE: u32 = 0x00B1_B104;
pub const LOD1_MODEL_HASH: u32 = 0x00F9_EFBB;
pub const LOD2_MODEL_HASH: u32 = 0x00F9_EFBC;
pub const LOD3_MODEL_HASH: u32 = 0x00F9_EFBD;
pub const LOD4_MODEL_HASH: u32 = 0x00F9_EFBE;
pub const LOT_MASK_HASH: u32 = 0x0CCB_7FD5;
pub const LOT_SIZE_HASH: u32 = 0x0CCB_7FC8;
/// C# `LotUnitOffset`/`LotOverlayBoxOffset`（Vector2，地面矩形相对模型的偏移）。
pub const LOT_OVERLAY_OFFSET_HASH: u32 = 0x0CCB_7FC9;
pub const LOT_PLACEMENT_HASH: u32 = 0x0DB7_FB17;
/// "Lot Textures"（0x0CCB7FD4）：地表共享纹理容器（纯纹理 RW4，如
/// 1024² DXT5 = 4×4 tile 图集），shader lotTextureSampler 的绑定源。
pub const LOT_TEXTURES_HASH: u32 = 0x0CCB_7FD4;

/// LOD1~LOD4 的 property hash（C# `PropertyConstants.UnitLOD1..4`）。
pub const LOD_MODEL_HASHES: [u32; 4] = [
    LOD1_MODEL_HASH,
    LOD2_MODEL_HASH,
    LOD3_MODEL_HASH,
    LOD4_MODEL_HASH,
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LotEditorDocument {
    pub properties: PropertyFile,
    pub model: Option<Key>,
    /// LOD1~LOD4 模型引用（各一条 Key 属性，可缺失）。
    pub model_lods: [Option<Key>; 4],
    pub lot_size: Option<[f32; 2]>,
    pub lot_offset: Option<[f32; 2]>,
    pub placement: Option<Transform>,
    pub lot_mask: Option<Key>,
    /// "Lot Textures" 地表共享纹理容器引用（0x0CCB7FD4）。
    pub lot_textures: Option<Key>,
    pub unknown_property_count: usize,
}

impl LotEditorDocument {
    pub fn from_property_file(properties: PropertyFile) -> Self {
        let model_lods = LOD_MODEL_HASHES.map(|hash| scalar_key(&properties, hash));
        let model = model_lods[0].clone();
        let lot_mask = scalar_key(&properties, LOT_MASK_HASH);
        let lot_textures = scalar_key(&properties, LOT_TEXTURES_HASH);
        let lot_size = value_vec2(&properties, LOT_SIZE_HASH);
        let lot_offset = value_vec2(&properties, LOT_OVERLAY_OFFSET_HASH);
        let placement = value_transform(&properties, LOT_PLACEMENT_HASH);
        let known = [
            LOD1_MODEL_HASH,
            LOD2_MODEL_HASH,
            LOD3_MODEL_HASH,
            LOD4_MODEL_HASH,
            LOT_MASK_HASH,
            LOT_SIZE_HASH,
            LOT_OVERLAY_OFFSET_HASH,
            LOT_PLACEMENT_HASH,
            LOT_TEXTURES_HASH,
        ];
        let unknown_property_count = properties
            .values
            .iter()
            .filter(|property| !known.contains(&property.hash))
            .count();
        Self {
            properties,
            model,
            model_lods,
            lot_size,
            lot_offset,
            placement,
            lot_mask,
            lot_textures,
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
    let value = match &properties.get(hash)?.kind {
        Kind::Scalar(value) => Some(value),
        // C# `PropertyFileArrayPropertyAttribute`：真实数据多为数组形态
        //（LotPlacementTransform[0]，见 ViewLotEditor.CreateLotModel）
        Kind::Array(values) => values.first(),
        Kind::Empty => None,
    }?;
    match value {
        Value::Transform(value) => Some(value.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Property, PropertyEncoding, PropType};
    use crate::{Kind, Value};

    fn prop(hash: u32, kind: Kind) -> Property {
        Property {
            hash,
            prop_type: crate::model::PropType::Transform,
            kind,
            encoding: PropertyEncoding::default(),
        }
    }

    fn transform_matrix(x: f32, y: f32) -> Transform {
        Transform {
            flags: 0,
            unknown: None,
            matrix: vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, x, y, 0.0],
        }
    }

    fn file(values: Vec<Property>) -> PropertyFile {
        let claimed_count = values.len() as u32;
        PropertyFile { values, claimed_count }
    }

    #[test]
    fn placement_read_from_array_and_scalar_forms() {
        // 真实数据（C# 数组属性）→ Kind::Array
        let array_form = file(vec![prop(
            LOT_PLACEMENT_HASH,
            Kind::Array(vec![Value::Transform(transform_matrix(10.0, 20.0))]),
        )]);
        let doc = LotEditorDocument::from_property_file(array_form);
        assert_eq!(doc.placement.as_ref().unwrap().matrix[9], 10.0);
        assert_eq!(doc.placement.as_ref().unwrap().matrix[10], 20.0);

        // 标量形态兜底
        let scalar_form = file(vec![prop(
            LOT_PLACEMENT_HASH,
            Kind::Scalar(Value::Transform(transform_matrix(5.0, 0.0))),
        )]);
        let doc = LotEditorDocument::from_property_file(scalar_form);
        assert_eq!(doc.placement.as_ref().unwrap().matrix[9], 5.0);
    }
}
