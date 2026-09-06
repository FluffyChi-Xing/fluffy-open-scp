//! Lot Unit 结构层：把扁平属性字典装配为强类型 Unit（灯光/效果/贴花/道具槽/
//! 路径点/生成器）。
//!
//! 装配约定与 C# `PropertyFileObjectCollection.Load` 一致：每个属性 hash 是
//! 一列并行数组，Unit #N 取各列的第 N 个元素；单列较短时该字段缺省，所有列
//! 在某下标都缺值时停止。hash 常量迁移自原
//! `SimCityPak/Views/AdvancedEditors/LotEditor/PropertyConstants.cs`。

use serde::Serialize;

use crate::model::Value;
use crate::PropertyFile;

// ---- Light（14 列并行数组） ----
pub const LIGHT_IDS: u32 = 0x0CAA_8F10;
pub const LIGHT_TYPES: u32 = 0x0CAA_8F11;
pub const LIGHT_TRANSFORMS: u32 = 0x0CAA_8F12;
pub const LIGHT_COLORS: u32 = 0x0CAA_8F13;
pub const LIGHT_RADII: u32 = 0x0CAA_8F14;
pub const LIGHT_INNER_RADII: u32 = 0x0CAA_8F15;
pub const LIGHT_DIFFUSE_LEVELS: u32 = 0x0CAA_8F16;
pub const LIGHT_DEBUG_NAMES: u32 = 0x0CAA_8F17;
pub const LIGHT_SPEC_LEVELS: u32 = 0x0D4C_96B4;
pub const LIGHT_LENGTHS: u32 = 0x0D4C_AD23;
pub const LIGHT_CULL_DISTANCES: u32 = 0x0E01_200F;
pub const LIGHT_FALLOFF_STARTS: u32 = 0x0E4D_F244;
pub const LIGHT_IS_VOLUMETRIC: u32 = 0x0E98_D445;
pub const LIGHT_VOL_STRENGTHS: u32 = 0x0EC4_567E;

/// 枚举值存于 Key.instance（C# `ViewLotEditor.LightTypes`）。
pub const LIGHT_TYPE_POINT: u32 = 0x75D4_C8CD;
pub const LIGHT_TYPE_SPOT: u32 = 0x2F0F_F9FD;
pub const LIGHT_TYPE_LINE: u32 = 0x0820_ABAF;

/// C# `ViewLotEditor.LightCullDistances`。
pub const CULL_NEAR: u32 = 0x4394_551B;
pub const CULL_MID: u32 = 0x467E_1EA9;
pub const CULL_FAR: u32 = 0x468F_679C;
pub const CULL_MAX: u32 = 0x3E7E_124D;

// ---- Effect ----
pub const EFFECT_IDS: u32 = 0x02A9_07B5;
pub const EFFECT_TRANSFORMS: u32 = 0x02A9_07B6;
pub const EFFECT_ALWAYS_ZERO: u32 = 0x02A9_07B9;
pub const EFFECT_REF_IDS: u32 = 0x02A9_07BB;
pub const EFFECT_ENABLED: u32 = 0x02A9_07BC;

// ---- Decal（3 组类别，hash = BASE + category） ----
pub const DECAL_ID_BASE: u32 = 0x0D10_9050;
pub const DECAL_TRANSFORM_BASE: u32 = 0x0D10_9060;
pub const DECAL_DEPTH_BASE: u32 = 0x0D10_9070;
pub const DECAL_MATERIAL_BASE: u32 = 0x0D10_9080;
pub const DECAL_CATEGORIES: usize = 3;

// ---- Prop / BinDrawSlot（14 分箱，各一列；C# 数组属性表实际有 15 个
// hash，`PropertyFileObjectCollectionArray(14)` 只用前 14 个） ----
pub const PROP_TRANSFORM_BASE: u32 = 0x0C12_EF30;
pub const PROP_SLOT_BASE: u32 = 0x0C12_EF40;
pub const PROP_BINS: usize = 14;

// ---- PathPoint / Path ----
pub const PATH_POINT_POINTS: u32 = 0x0CAA_680D;
pub const PATH_POINT_TANGENTS: u32 = 0x0CB0_0ED8;
pub const PATH_POINT_INDICES: u32 = 0x0CAA_6832;
/// Int32 对（点区间），C# `UnitFile.UnitPath`。
pub const PATH_PAIRS: u32 = 0x0CAA_6841;

// ---- SimsSpawner ----
pub const SPAWNER_IDS: u32 = 0x0E1B_AC61;
pub const SPAWNER_TRANSFORMS: u32 = 0x0E1B_AC62;

/// WPF Matrix3D 行主序 12 floats：行 1-3 为基向量，行 4 为平移。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitTransform {
    pub matrix: [f32; 12],
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitKey {
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
}

/// 只读属性面板的一行（该 Unit 消费到的、在此下标有值的属性）。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitField {
    pub hash: u32,
    pub type_name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LotUnit {
    #[serde(rename_all = "camelCase")]
    Light {
        index: usize,
        transform: Option<UnitTransform>,
        /// "Point" | "Spot" | "Line"；未知 instance 为 None。
        light_type: Option<&'static str>,
        color: Option<[f32; 3]>,
        outer_radius: Option<f32>,
        inner_radius: Option<f32>,
        diffuse: Option<f32>,
        length: Option<f32>,
        cull_distance: Option<&'static str>,
        is_volumetric: Option<bool>,
        debug_name: Option<String>,
        fields: Vec<UnitField>,
    },
    #[serde(rename_all = "camelCase")]
    Effect {
        index: usize,
        transform: Option<UnitTransform>,
        effect_id: Option<UnitKey>,
        enabled: Option<bool>,
        fields: Vec<UnitField>,
    },
    #[serde(rename_all = "camelCase")]
    Decal {
        index: usize,
        category: usize,
        transform: Option<UnitTransform>,
        /// C# hack：Scale 存于 Transform.Unknown（flags == 15）。
        scale: Option<f32>,
        depth: Option<f32>,
        material_data: Option<[f32; 3]>,
        fields: Vec<UnitField>,
    },
    #[serde(rename_all = "camelCase")]
    Prop {
        /// 分箱内下标；C# Billboard 序号用的是 bin（`Prop.bin`）。
        index: usize,
        bin: usize,
        transform: Option<UnitTransform>,
        slot: Option<i32>,
        fields: Vec<UnitField>,
    },
    #[serde(rename_all = "camelCase")]
    PathPoint {
        index: usize,
        point: Option<[f32; 3]>,
        tangent: Option<[f32; 3]>,
        point_index: Option<i32>,
        fields: Vec<UnitField>,
    },
    #[serde(rename_all = "camelCase")]
    Spawner {
        index: usize,
        transform: Option<UnitTransform>,
        id: Option<UnitKey>,
        fields: Vec<UnitField>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LotUnits {
    pub units: Vec<LotUnit>,
    pub path_pairs: Vec<i32>,
    pub diagnostics: Vec<String>,
}

/// 把属性字典装配为 Unit 列表（只读视图；M-PE2 在此旁加展平回写）。
pub fn assemble_units(file: &PropertyFile) -> LotUnits {
    let mut out = LotUnits::default();
    assemble_lights(file, &mut out);
    assemble_effects(file, &mut out);
    assemble_spawners(file, &mut out);
    assemble_path_points(file, &mut out);
    assemble_props(file, &mut out);
    assemble_decals(file, &mut out);
    if let Some(values) = column(file, PATH_PAIRS) {
        out.path_pairs = values
            .iter()
            .filter_map(|value| match value {
                Value::Int32(v) => Some(*v),
                _ => None,
            })
            .collect();
    }
    out
}

fn column(file: &PropertyFile, hash: u32) -> Option<&[Value]> {
    file.get(hash).and_then(|property| property.array())
}

fn value_at<'a>(values: Option<&'a [Value]>, index: usize) -> Option<&'a Value> {
    values.and_then(|values| values.get(index))
}

fn column_count(columns: &[Option<&[Value]>]) -> usize {
    columns
        .iter()
        .flatten()
        .map(|column| column.len())
        .max()
        .unwrap_or(0)
}

fn parallel_count(file: &PropertyFile, hashes: &[u32]) -> usize {
    let columns: Vec<Option<&[Value]>> = hashes.iter().map(|hash| column(file, *hash)).collect();
    column_count(&columns)
}

fn collect_fields(file: &PropertyFile, hashes: &[u32], index: usize) -> Vec<UnitField> {
    let mut fields = Vec::new();
    for hash in hashes {
        if let Some(value) = value_at(column(file, *hash), index) {
            let type_name = file
                .get(*hash)
                .map(|property| property.prop_type.name().to_string())
                .unwrap_or_default();
            fields.push(UnitField {
                hash: *hash,
                type_name,
                value: value.to_string(),
            });
        }
    }
    fields
}

fn unit_transform(
    file: &PropertyFile,
    hash: u32,
    index: usize,
    diagnostics: &mut Vec<String>,
) -> Option<UnitTransform> {
    let value = value_at(column(file, hash), index)?;
    let Value::Transform(transform) = value else {
        return None;
    };
    match transform.matrix.as_slice().try_into() {
        Ok(matrix) => Some(UnitTransform { matrix }),
        Err(_) => {
            diagnostics.push(format!(
                "property 0x{hash:08X}[{index}]: transform carries {} floats, expected 12; unit placed at origin",
                transform.matrix.len()
            ));
            None
        }
    }
}

fn unit_key(file: &PropertyFile, hash: u32, index: usize) -> Option<UnitKey> {
    match value_at(column(file, hash), index)? {
        Value::Key(key) => Some(UnitKey {
            type_id: key.type_id,
            group: key.group,
            instance: key.instance,
        }),
        _ => None,
    }
}

fn unit_float(file: &PropertyFile, hash: u32, index: usize) -> Option<f32> {
    match value_at(column(file, hash), index)? {
        Value::Float(v) => Some(*v),
        _ => None,
    }
}

fn unit_bool(file: &PropertyFile, hash: u32, index: usize) -> Option<bool> {
    match value_at(column(file, hash), index)? {
        Value::Bool(v) => Some(*v),
        _ => None,
    }
}

fn unit_vec3(file: &PropertyFile, hash: u32, index: usize) -> Option<[f32; 3]> {
    match value_at(column(file, hash), index)? {
        Value::Vector3(v) => Some(*v),
        _ => None,
    }
}

fn light_type_name(instance: u32) -> Option<&'static str> {
    match instance {
        LIGHT_TYPE_POINT => Some("Point"),
        LIGHT_TYPE_SPOT => Some("Spot"),
        LIGHT_TYPE_LINE => Some("Line"),
        _ => None,
    }
}

fn cull_distance_name(instance: u32) -> Option<&'static str> {
    match instance {
        CULL_NEAR => Some("Near"),
        CULL_MID => Some("Mid"),
        CULL_FAR => Some("Far"),
        CULL_MAX => Some("Max"),
        _ => None,
    }
}

const LIGHT_COLUMN_HASHES: [u32; 14] = [
    LIGHT_IDS,
    LIGHT_TYPES,
    LIGHT_TRANSFORMS,
    LIGHT_COLORS,
    LIGHT_RADII,
    LIGHT_INNER_RADII,
    LIGHT_DIFFUSE_LEVELS,
    LIGHT_DEBUG_NAMES,
    LIGHT_SPEC_LEVELS,
    LIGHT_LENGTHS,
    LIGHT_CULL_DISTANCES,
    LIGHT_FALLOFF_STARTS,
    LIGHT_IS_VOLUMETRIC,
    LIGHT_VOL_STRENGTHS,
];

fn assemble_lights(file: &PropertyFile, out: &mut LotUnits) {
    for index in 0..parallel_count(file, &LIGHT_COLUMN_HASHES) {
        let present = LIGHT_COLUMN_HASHES
            .iter()
            .any(|hash| value_at(column(file, *hash), index).is_some());
        if !present {
            continue;
        }
        let light_type = match unit_key(file, LIGHT_TYPES, index) {
            Some(key) => match light_type_name(key.instance) {
                Some(name) => Some(name),
                None => {
                    out.diagnostics.push(format!(
                        "light[{index}]: unknown light type instance 0x{:08X}",
                        key.instance
                    ));
                    None
                }
            },
            None => None,
        };
        let cull_distance = match unit_key(file, LIGHT_CULL_DISTANCES, index) {
            Some(key) => match cull_distance_name(key.instance) {
                Some(name) => Some(name),
                None => {
                    out.diagnostics.push(format!(
                        "light[{index}]: unknown cull distance instance 0x{:08X}",
                        key.instance
                    ));
                    None
                }
            },
            None => None,
        };
        let color = match value_at(column(file, LIGHT_COLORS), index) {
            Some(Value::ColorRgb { r, g, b }) => Some([*r, *g, *b]),
            Some(Value::ColorRgba { r, g, b, .. }) => Some([*r, *g, *b]),
            _ => None,
        };
        let debug_name = match value_at(column(file, LIGHT_DEBUG_NAMES), index) {
            Some(Value::String8(name)) => Some(name.clone()),
            _ => None,
        };
        out.units.push(LotUnit::Light {
            index,
            transform: unit_transform(file, LIGHT_TRANSFORMS, index, &mut out.diagnostics),
            light_type,
            color,
            outer_radius: unit_float(file, LIGHT_RADII, index),
            inner_radius: unit_float(file, LIGHT_INNER_RADII, index),
            diffuse: unit_float(file, LIGHT_DIFFUSE_LEVELS, index),
            length: unit_float(file, LIGHT_LENGTHS, index),
            cull_distance,
            is_volumetric: unit_bool(file, LIGHT_IS_VOLUMETRIC, index),
            debug_name,
            fields: collect_fields(file, &LIGHT_COLUMN_HASHES, index),
        });
    }
}

const EFFECT_COLUMN_HASHES: [u32; 5] = [
    EFFECT_IDS,
    EFFECT_TRANSFORMS,
    EFFECT_ALWAYS_ZERO,
    EFFECT_REF_IDS,
    EFFECT_ENABLED,
];

fn assemble_effects(file: &PropertyFile, out: &mut LotUnits) {
    for index in 0..parallel_count(file, &EFFECT_COLUMN_HASHES) {
        let present = EFFECT_COLUMN_HASHES
            .iter()
            .any(|hash| value_at(column(file, *hash), index).is_some());
        if !present {
            continue;
        }
        out.units.push(LotUnit::Effect {
            index,
            transform: unit_transform(file, EFFECT_TRANSFORMS, index, &mut out.diagnostics),
            effect_id: unit_key(file, EFFECT_REF_IDS, index),
            enabled: unit_bool(file, EFFECT_ENABLED, index),
            fields: collect_fields(file, &EFFECT_COLUMN_HASHES, index),
        });
    }
}

fn assemble_spawners(file: &PropertyFile, out: &mut LotUnits) {
    let hashes = [SPAWNER_IDS, SPAWNER_TRANSFORMS];
    for index in 0..parallel_count(file, &hashes) {
        let present = hashes
            .iter()
            .any(|hash| value_at(column(file, *hash), index).is_some());
        if !present {
            continue;
        }
        out.units.push(LotUnit::Spawner {
            index,
            transform: unit_transform(file, SPAWNER_TRANSFORMS, index, &mut out.diagnostics),
            id: unit_key(file, SPAWNER_IDS, index),
            fields: collect_fields(file, &hashes, index),
        });
    }
}

fn assemble_path_points(file: &PropertyFile, out: &mut LotUnits) {
    let hashes = [
        PATH_POINT_POINTS,
        PATH_POINT_TANGENTS,
        PATH_POINT_INDICES,
    ];
    for index in 0..parallel_count(file, &hashes) {
        let present = hashes
            .iter()
            .any(|hash| value_at(column(file, *hash), index).is_some());
        if !present {
            continue;
        }
        out.units.push(LotUnit::PathPoint {
            index,
            point: unit_vec3(file, PATH_POINT_POINTS, index),
            tangent: unit_vec3(file, PATH_POINT_TANGENTS, index),
            point_index: match value_at(column(file, PATH_POINT_INDICES), index) {
                Some(Value::Int32(v)) => Some(*v),
                _ => None,
            },
            fields: collect_fields(file, &hashes, index),
        });
    }
}

fn assemble_props(file: &PropertyFile, out: &mut LotUnits) {
    for bin in 0..PROP_BINS {
        let transform_hash = PROP_TRANSFORM_BASE + bin as u32;
        let slot_hash = PROP_SLOT_BASE + bin as u32;
        let hashes = [transform_hash, slot_hash];
        for index in 0..parallel_count(file, &hashes) {
            let present = hashes
                .iter()
                .any(|hash| value_at(column(file, *hash), index).is_some());
            if !present {
                continue;
            }
            let slot = match value_at(column(file, slot_hash), index) {
                Some(Value::Int32(v)) => Some(*v),
                _ => None,
            };
            out.units.push(LotUnit::Prop {
                index,
                bin,
                transform: unit_transform(file, transform_hash, index, &mut out.diagnostics),
                slot,
                fields: collect_fields(file, &hashes, index),
            });
        }
    }
}

fn assemble_decals(file: &PropertyFile, out: &mut LotUnits) {
    for category in 0..DECAL_CATEGORIES {
        let id_hash = DECAL_ID_BASE + category as u32;
        let transform_hash = DECAL_TRANSFORM_BASE + category as u32;
        let depth_hash = DECAL_DEPTH_BASE + category as u32;
        let material_hash = DECAL_MATERIAL_BASE + category as u32;
        let hashes = [id_hash, transform_hash, depth_hash, material_hash];
        for index in 0..parallel_count(file, &hashes) {
            let present = hashes
                .iter()
                .any(|hash| value_at(column(file, *hash), index).is_some());
            if !present {
                continue;
            }
            let transform = unit_transform(file, transform_hash, index, &mut out.diagnostics);
            // Scale hack：flags == 15 时 Transform.Unknown 即贴花 Scale。
            let scale = file
                .get(transform_hash)
                .and_then(|property| property.array())
                .and_then(|values| values.get(index))
                .and_then(|value| match value {
                    Value::Transform(transform) => transform.unknown,
                    _ => None,
                });
            out.units.push(LotUnit::Decal {
                index,
                category,
                transform,
                scale,
                depth: unit_float(file, depth_hash, index),
                material_data: unit_vec3(file, material_hash, index),
                fields: collect_fields(file, &hashes, index),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Key;
    use crate::model::{Kind, PropType, Property, PropertyEncoding};

    fn prop(hash: u32, prop_type: PropType, kind: Kind) -> Property {
        Property {
            hash,
            prop_type,
            kind,
            encoding: PropertyEncoding::default(),
        }
    }

    fn float_array(hash: u32, values: &[f32]) -> Property {
        prop(
            hash,
            PropType::Float,
            Kind::Array(values.iter().map(|v| Value::Float(*v)).collect()),
        )
    }

    fn key_array(hash: u32, instances: &[u32]) -> Property {
        prop(
            hash,
            PropType::Key,
            Kind::Array(
                instances
                    .iter()
                    .map(|instance| {
                        Value::Key(Key {
                            instance: *instance,
                            type_id: 0,
                            group: 0,
                        })
                    })
                    .collect(),
            ),
        )
    }

    fn transform_value(unknown: Option<f32>, matrix: &[f32]) -> Value {
        Value::Transform(crate::Transform {
            flags: if unknown.is_some() { 15 } else { 0 },
            unknown,
            matrix: matrix.to_vec(),
        })
    }

    fn matrix_translation(x: f32, y: f32, z: f32) -> Vec<f32> {
        let mut m = [0.0f32; 12];
        m[0] = 1.0;
        m[4] = 1.0;
        m[8] = 1.0;
        m[9] = x;
        m[10] = y;
        m[11] = z;
        m.to_vec()
    }

    fn file_of(values: Vec<Property>) -> PropertyFile {
        let claimed_count = values.len() as u32;
        PropertyFile {
            values,
            claimed_count,
        }
    }

    #[test]
    fn lights_parallel_columns_with_unequal_lengths() {
        let file = file_of(vec![
            key_array(
                LIGHT_TYPES,
                &[LIGHT_TYPE_POINT, LIGHT_TYPE_SPOT, LIGHT_TYPE_LINE],
            ),
            float_array(LIGHT_RADII, &[2.5, 3.0]),
        ]);
        let units = assemble_units(&file);
        assert_eq!(units.units.len(), 3, "最短列缺值只影响字段，不影响计数");
        for (index, unit) in units.units.iter().enumerate() {
            let LotUnit::Light {
                index: unit_index,
                light_type,
                outer_radius,
                fields,
                ..
            } = unit
            else {
                panic!("expected light, got {unit:?}");
            };
            assert_eq!(*unit_index, index);
            assert_eq!(light_type, &Some(["Point", "Spot", "Line"][index]));
            if index < 2 {
                assert_eq!(outer_radius, &Some([2.5, 3.0][index]));
            } else {
                assert_eq!(outer_radius, &None);
                assert_eq!(fields.len(), 1, "仅 Type 列在此下标有值");
            }
        }
    }

    #[test]
    fn empty_column_gap_stops_collection() {
        // 唯一列有 2 个值 → 2 个 unit；无"空档跳过"语义需要额外验证：
        // 任一下标全部列都缺值时不再产出（这里通过单列长度即可覆盖）。
        let file = file_of(vec![float_array(LIGHT_LENGTHS, &[4.0, 6.0])]);
        let units = assemble_units(&file);
        assert_eq!(units.units.len(), 2);
        assert!(matches!(units.units[0], LotUnit::Light { length: Some(4.0), .. }));
    }

    #[test]
    fn spot_light_geometry_fields_mapped() {
        let file = file_of(vec![
            key_array(LIGHT_TYPES, &[LIGHT_TYPE_SPOT]),
            float_array(LIGHT_RADII, &[5.0]),
            float_array(LIGHT_INNER_RADII, &[1.5]),
            float_array(LIGHT_LENGTHS, &[9.0]),
            key_array(LIGHT_CULL_DISTANCES, &[CULL_MID]),
            prop(
                LIGHT_COLORS,
                PropType::ColorRgb,
                Kind::Array(vec![Value::ColorRgb {
                    r: 1.0,
                    g: 0.6,
                    b: 0.2,
                }]),
            ),
            prop(
                LIGHT_IS_VOLUMETRIC,
                PropType::Bool,
                Kind::Array(vec![Value::Bool(true)]),
            ),
            prop(
                LIGHT_DEBUG_NAMES,
                PropType::String8,
                Kind::Array(vec![Value::String8("window glow".into())]),
            ),
        ]);
        let units = assemble_units(&file);
        let LotUnit::Light {
            light_type,
            color,
            outer_radius,
            inner_radius,
            length,
            cull_distance,
            is_volumetric,
            debug_name,
            fields,
            ..
        } = &units.units[0]
        else {
            panic!("expected light");
        };
        assert_eq!(light_type.as_deref(), Some("Spot"));
        assert_eq!(color, &Some([1.0, 0.6, 0.2]));
        assert_eq!(outer_radius, &Some(5.0));
        assert_eq!(inner_radius, &Some(1.5));
        assert_eq!(length, &Some(9.0));
        assert_eq!(cull_distance.as_deref(), Some("Mid"));
        assert_eq!(is_volumetric, &Some(true));
        assert_eq!(debug_name.as_deref(), Some("window glow"));
        assert_eq!(fields.len(), 8);
    }

    #[test]
    fn unknown_light_type_and_cull_report_diagnostics() {
        let file = file_of(vec![
            key_array(LIGHT_TYPES, &[0x1234_5678]),
            key_array(LIGHT_CULL_DISTANCES, &[0x8765_4321]),
        ]);
        let units = assemble_units(&file);
        assert_eq!(units.units.len(), 1);
        let LotUnit::Light {
            light_type,
            cull_distance,
            ..
        } = &units.units[0]
        else {
            panic!("expected light");
        };
        assert_eq!(light_type, &None);
        assert_eq!(cull_distance, &None);
        assert_eq!(units.diagnostics.len(), 2);
    }

    #[test]
    fn decal_scale_read_from_unknown_hack() {
        let matrix = matrix_translation(1.0, 2.0, 3.0);
        let file = file_of(vec![
            prop(
                DECAL_ID_BASE,
                PropType::Key,
                Kind::Array(vec![Value::Key(Key {
                    instance: 0xFEED,
                    type_id: 0,
                    group: 0,
                })]),
            ),
            prop(
                DECAL_TRANSFORM_BASE,
                PropType::Transform,
                Kind::Array(vec![transform_value(Some(4.5), &matrix)]),
            ),
            float_array(DECAL_DEPTH_BASE, &[0.25]),
            prop(
                DECAL_MATERIAL_BASE,
                PropType::Vector3,
                Kind::Array(vec![Value::Vector3([1.0, 0.0, 0.5])]),
            ),
        ]);
        let units = assemble_units(&file);
        let LotUnit::Decal {
            index,
            category,
            transform,
            scale,
            depth,
            material_data,
            fields,
            ..
        } = &units.units[0]
        else {
            panic!("expected decal");
        };
        assert_eq!((*index, *category), (0, 0));
        assert_eq!(scale, &Some(4.5));
        assert_eq!(transform.as_ref().map(|t| t.matrix[11]), Some(3.0));
        assert_eq!(depth, &Some(0.25));
        assert_eq!(material_data, &Some([1.0, 0.0, 0.5]));
        assert_eq!(fields.len(), 4);
    }

    #[test]
    fn prop_bins_select_hash_rows() {
        let file = file_of(vec![
            prop(
                PROP_TRANSFORM_BASE, // bin 0
                PropType::Transform,
                Kind::Array(vec![transform_value(None, &matrix_translation(0.0, 0.0, 1.0))]),
            ),
            prop(
                PROP_SLOT_BASE, // bin 0
                PropType::Int32,
                Kind::Array(vec![Value::Int32(0)]),
            ),
            prop(
                PROP_TRANSFORM_BASE + 13, // bin 13
                PropType::Transform,
                Kind::Array(vec![
                    transform_value(None, &matrix_translation(1.0, 0.0, 0.0)),
                    transform_value(None, &matrix_translation(2.0, 0.0, 0.0)),
                ]),
            ),
            prop(
                PROP_SLOT_BASE + 13, // bin 13
                PropType::Int32,
                Kind::Array(vec![Value::Int32(0), Value::Int32(1)]),
            ),
        ]);
        let units = assemble_units(&file);
        assert_eq!(units.units.len(), 3);
        let LotUnit::Prop { bin, slot, .. } = &units.units[0] else {
            panic!("expected prop");
        };
        assert_eq!((*bin, *slot), (0, Some(0)));
        let LotUnit::Prop { index, bin, slot, .. } = &units.units[2] else {
            panic!("expected prop");
        };
        assert_eq!((*index, *bin, *slot), (1, 13, Some(1)));
    }

    #[test]
    fn path_points_and_pairs() {
        let file = file_of(vec![
            prop(
                PATH_POINT_POINTS,
                PropType::Vector3,
                Kind::Array(vec![Value::Vector3([0.0, 0.0, 0.0]), Value::Vector3([4.0, 0.0, 0.0])]),
            ),
            prop(
                PATH_POINT_TANGENTS,
                PropType::Vector3,
                Kind::Array(vec![Value::Vector3([1.0, 0.0, 0.0]), Value::Vector3([1.0, 0.0, 0.0])]),
            ),
            prop(
                PATH_POINT_INDICES,
                PropType::Int32,
                Kind::Array(vec![Value::Int32(0), Value::Int32(1)]),
            ),
            prop(
                PATH_PAIRS,
                PropType::Int32,
                Kind::Array(vec![Value::Int32(0), Value::Int32(1)]),
            ),
        ]);
        let units = assemble_units(&file);
        assert_eq!(units.units.len(), 2);
        let LotUnit::PathPoint {
            point, point_index, ..
        } = &units.units[1]
        else {
            panic!("expected path point");
        };
        assert_eq!(point, &Some([4.0, 0.0, 0.0]));
        assert_eq!(point_index, &Some(1));
        assert_eq!(units.path_pairs, vec![0, 1]);
    }

    #[test]
    fn short_transform_yields_none_and_diagnostic() {
        let file = file_of(vec![
            prop(
                SPAWNER_IDS,
                PropType::Key,
                Kind::Array(vec![Value::Key(Key {
                    instance: 0xBEEF,
                    type_id: 0,
                    group: 0,
                })]),
            ),
            prop(
                SPAWNER_TRANSFORMS,
                PropType::Transform,
                // flags 0x0C → 3 floats，无有效摆位。
                Kind::Array(vec![Value::Transform(crate::Transform {
                    flags: 0x0C,
                    unknown: None,
                    matrix: vec![1.0, 2.0, 3.0],
                })]),
            ),
        ]);
        let units = assemble_units(&file);
        let LotUnit::Spawner {
            transform, id, ..
        } = &units.units[0]
        else {
            panic!("expected spawner");
        };
        assert_eq!(transform, &None);
        assert_eq!(id.as_ref().map(|key| key.instance), Some(0xBEEF));
        assert_eq!(units.diagnostics.len(), 1);
    }

    #[test]
    fn spawner_and_effect_assembled() {
        let matrix = matrix_translation(0.0, 5.0, 0.0);
        let file = file_of(vec![
            prop(
                SPAWNER_IDS,
                PropType::Key,
                Kind::Array(vec![Value::Key(Key {
                    instance: 1,
                    type_id: 0,
                    group: 0,
                })]),
            ),
            prop(
                SPAWNER_TRANSFORMS,
                PropType::Transform,
                Kind::Array(vec![transform_value(None, &matrix)]),
            ),
            prop(
                EFFECT_REF_IDS,
                PropType::Key,
                Kind::Array(vec![Value::Key(Key {
                    instance: 0x777,
                    type_id: 0,
                    group: 0,
                })]),
            ),
            prop(
                EFFECT_TRANSFORMS,
                PropType::Transform,
                Kind::Array(vec![transform_value(None, &matrix)]),
            ),
            prop(
                EFFECT_ENABLED,
                PropType::Bool,
                Kind::Array(vec![Value::Bool(false)]),
            ),
        ]);
        let units = assemble_units(&file);
        assert_eq!(units.units.len(), 2);
        let LotUnit::Effect {
            effect_id, enabled, ..
        } = &units.units[0]
        else {
            panic!("expected effect");
        };
        assert_eq!(effect_id.as_ref().map(|key| key.instance), Some(0x777));
        assert_eq!(enabled, &Some(false));
        assert!(matches!(
            units.units[1],
            LotUnit::Spawner { index: 0, .. }
        ));
    }
}
