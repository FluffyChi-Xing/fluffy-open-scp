//! Decal Dictionary（贴花图集字典）解析。
//!
//! 迁移自 C# `SimCityPak\Views\AdvancedEditors\DecalDictionary\`。SimCity 的
//! 「decal atlas」不是一张图片，而是一个普通 Property 资源（`0x00B1B104`），
//! 由 **GroupContainer 低 16 位**区分：`0xB185` / `0x1651` / `0x1652`
//! （C# `PropertyFileTypeIds.DecalAtlas{1,2,3}`，选择器见
//! `TemplateSelectors/ViewSelector.cs`）。
//!
//! 载荷是 **列式并行数组**：没有条目计数字段，条目 *i* 由 7 个数组的下标 *i*
//! 组成。C# 侧靠 `PropertyFileObjectCollectionAttribute` 按同一个下标循环装配，
//! 长度不一致时会抛 `IndexOutOfRangeException`。
//!
//! ```text
//! 字典级标量：
//!   0x0CE5EF4E  MaterialId    Key
//!   0x0CE5EF4F  TextureSize   Vector2
//!   0x0CE5EF60  AtlasSize     Vector2
//! 条目级数组（长度应一致）：
//!   0x0CE5EF50  ID            Key
//!   0x0CE5EF53  AspectRatio   Float
//!   0x0CE5EF58  RasterFileID  Key   -> 0x2F4E681C Raster 资源的 instance
//!   0x0CE5EF5C  Color1        Vector4
//!   0x0CE5EF5D  Color2        Vector4
//!   0x0CE5EF5E  Color3        Vector4
//!   0x0CE5EF5F  Color4        Vector4
//! ```

use crate::{Key, PropertyFile, Result, Value};

/// 三种 Decal Atlas 的 GroupContainer 低 16 位值。
pub const DECAL_ATLAS_INSTANCE_TYPES: [u16; 3] = [0xb185, 0x1651, 0x1652];

pub const DECAL_MATERIAL_HASH: u32 = 0x0ce5_ef4e;
pub const DECAL_TEXTURE_SIZE_HASH: u32 = 0x0ce5_ef4f;
pub const DECAL_ATLAS_SIZE_HASH: u32 = 0x0ce5_ef60;

pub const DECAL_ENTRY_ID_HASH: u32 = 0x0ce5_ef50;
pub const DECAL_ENTRY_ASPECT_RATIO_HASH: u32 = 0x0ce5_ef53;
pub const DECAL_ENTRY_RASTER_HASH: u32 = 0x0ce5_ef58;
pub const DECAL_ENTRY_COLOR_HASHES: [u32; 4] =
    [0x0ce5_ef5c, 0x0ce5_ef5d, 0x0ce5_ef5e, 0x0ce5_ef5f];

/// 7 个条目级数组，顺序即 UI 展示顺序。
pub const DECAL_ENTRY_ARRAY_HASHES: [u32; 7] = [
    DECAL_ENTRY_ID_HASH,
    DECAL_ENTRY_ASPECT_RATIO_HASH,
    DECAL_ENTRY_RASTER_HASH,
    DECAL_ENTRY_COLOR_HASHES[0],
    DECAL_ENTRY_COLOR_HASHES[1],
    DECAL_ENTRY_COLOR_HASHES[2],
    DECAL_ENTRY_COLOR_HASHES[3],
];

/// 该 GroupContainer 是否属于 Decal Atlas 家族（只看低 16 位，与 C# 一致）。
pub fn is_decal_dictionary_group(group: u32) -> bool {
    DECAL_ATLAS_INSTANCE_TYPES.contains(&(group as u16))
}

/// 该属性文件是否具备字典结构（任一字典级或条目级 hash 存在）。
pub fn looks_like_dictionary(file: &PropertyFile) -> bool {
    DECAL_ENTRY_ARRAY_HASHES
        .iter()
        .copied()
        .chain([
            DECAL_MATERIAL_HASH,
            DECAL_TEXTURE_SIZE_HASH,
            DECAL_ATLAS_SIZE_HASH,
        ])
        .any(|hash| file.get(hash).is_some())
}

/// 一个贴花条目：并行数组同一下标处的取值。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DecalEntry {
    pub index: usize,
    pub id: Option<Key>,
    /// 指向 `0x2F4E681C` Raster 资源的引用。
    pub raster: Option<Key>,
    pub aspect_ratio: Option<f32>,
    /// Color1..Color4，顺序与 C# 一致。
    pub colors: [Option<[f32; 4]>; 4],
}

impl DecalEntry {
    /// 四个颜色转换为预览用 RGBA8。
    ///
    /// 存储值是 **线性值的一半**：C# 侧存回时 `X = color.ScR / 2`、显示时
    /// `Color.FromScRgb(1, X*2, ...)`，因此 `X*2` 是线性分量，需做 sRGB
    /// 伽马编码才能得到与原 SCP 一致的显示色。W 分量在预览路径未使用。
    pub fn colors_rgba8(&self) -> Option<[[u8; 4]; 4]> {
        let mut out = [[0u8; 4]; 4];
        for (slot, color) in self.colors.iter().enumerate() {
            let [x, y, z, _w] = (*color)?;
            out[slot] = [
                linear_to_srgb_u8(x * 2.0),
                linear_to_srgb_u8(y * 2.0),
                linear_to_srgb_u8(z * 2.0),
                255,
            ];
        }
        Some(out)
    }
}

/// 线性分量 → sRGB 8bit（IEC 61966-2-1 传递函数）。
fn linear_to_srgb_u8(linear: f32) -> u8 {
    let linear = if linear.is_finite() { linear.clamp(0.0, 1.0) } else { 0.0 };
    let srgb = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round().clamp(0.0, 255.0) as u8
}

/// 一个解析后的 Decal Dictionary。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct DecalDictionary {
    pub material: Option<Key>,
    pub texture_size: Option<[f32; 2]>,
    pub atlas_size: Option<[f32; 2]>,
    pub entries: Vec<DecalEntry>,
    /// 每个条目级数组的 `(hash, 长度)`，用于诊断长度不一致。
    pub array_lengths: Vec<(u32, Option<usize>)>,
    /// 所有存在的条目数组长度是否一致（C# 装配的前提条件）。
    pub uniform_arrays: bool,
}

impl DecalDictionary {
    /// 解析属性文件。容忍并行数组长度不一致：条目数取各数组长度的最大值，
    /// 短数组越界处记为 `None`（C# 会直接抛异常，这里选择对只读浏览更宽容）。
    pub fn parse(data: &[u8]) -> Result<DecalDictionary> {
        let file = PropertyFile::parse(data)?;
        Ok(Self::from_file(&file))
    }

    pub fn from_file(file: &PropertyFile) -> DecalDictionary {
        let array_lengths: Vec<(u32, Option<usize>)> = DECAL_ENTRY_ARRAY_HASHES
            .iter()
            .map(|hash| (*hash, array(file, *hash).map(<[Value]>::len)))
            .collect();

        let present: Vec<usize> = array_lengths.iter().filter_map(|(_, len)| *len).collect();
        let uniform_arrays = present.windows(2).all(|pair| pair[0] == pair[1]);
        let count = present.iter().copied().max().unwrap_or(0);

        let entries = (0..count)
            .map(|index| DecalEntry {
                index,
                id: key_at(file, DECAL_ENTRY_ID_HASH, index),
                raster: key_at(file, DECAL_ENTRY_RASTER_HASH, index),
                aspect_ratio: float_at(file, DECAL_ENTRY_ASPECT_RATIO_HASH, index),
                colors: DECAL_ENTRY_COLOR_HASHES.map(|hash| vector4_at(file, hash, index)),
            })
            .collect();

        DecalDictionary {
            material: scalar_key(file, DECAL_MATERIAL_HASH),
            texture_size: scalar_vector2(file, DECAL_TEXTURE_SIZE_HASH),
            atlas_size: scalar_vector2(file, DECAL_ATLAS_SIZE_HASH),
            entries,
            array_lengths,
            uniform_arrays,
        }
    }
}

fn array<'a>(file: &'a PropertyFile, hash: u32) -> Option<&'a [Value]> {
    file.get(hash).and_then(|property| property.array())
}

fn scalar_key(file: &PropertyFile, hash: u32) -> Option<Key> {
    match file.get(hash)?.scalar()? {
        Value::Key(key) => Some(*key),
        _ => None,
    }
}

fn scalar_vector2(file: &PropertyFile, hash: u32) -> Option<[f32; 2]> {
    match file.get(hash)?.scalar()? {
        Value::Vector2(values) => Some(*values),
        _ => None,
    }
}

fn key_at(file: &PropertyFile, hash: u32, index: usize) -> Option<Key> {
    match array(file, hash)?.get(index)? {
        Value::Key(key) => Some(*key),
        _ => None,
    }
}

fn float_at(file: &PropertyFile, hash: u32, index: usize) -> Option<f32> {
    match array(file, hash)?.get(index)? {
        Value::Float(value) => Some(*value),
        _ => None,
    }
}

fn vector4_at(file: &PropertyFile, hash: u32, index: usize) -> Option<[f32; 4]> {
    match array(file, hash)?.get(index)? {
        Value::Vector4(values) => Some(*values),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Kind, PropType, Property, PropertyEncoding};

    fn array_property(hash: u32, prop_type: PropType, values: Vec<Value>) -> Property {
        Property {
            hash,
            prop_type,
            kind: Kind::Array(values),
            encoding: PropertyEncoding::default(),
        }
    }

    fn scalar_property(hash: u32, prop_type: PropType, value: Value) -> Property {
        Property {
            hash,
            prop_type,
            kind: Kind::Scalar(value),
            encoding: PropertyEncoding::default(),
        }
    }

    fn key(instance: u32) -> Value {
        Value::Key(Key {
            instance,
            type_id: 0x2f4e_681c,
            group: 0,
        })
    }

    fn file_from(properties: Vec<Property>) -> PropertyFile {
        PropertyFile {
            claimed_count: properties.len() as u32,
            values: properties,
        }
    }

    /// 三个条目、七个等长数组的最小字典。
    fn sample_file() -> PropertyFile {
        file_from(vec![
            scalar_property(
                DECAL_MATERIAL_HASH,
                PropType::Key,
                Value::Key(Key {
                    instance: 0xe539_0a98,
                    type_id: 0x2f4e_681b,
                    group: 0,
                }),
            ),
            scalar_property(
                DECAL_TEXTURE_SIZE_HASH,
                PropType::Vector2,
                Value::Vector2([32.0, 32.0]),
            ),
            scalar_property(
                DECAL_ATLAS_SIZE_HASH,
                PropType::Vector2,
                Value::Vector2([512.0, 512.0]),
            ),
            array_property(
                DECAL_ENTRY_ID_HASH,
                PropType::Key,
                vec![key(0xb059_45fb), key(0x224a_5eba), key(0xc392_1bf9)],
            ),
            array_property(
                DECAL_ENTRY_ASPECT_RATIO_HASH,
                PropType::Float,
                vec![
                    Value::Float(1.0),
                    Value::Float(0.5),
                    Value::Float(1.5),
                ],
            ),
            array_property(
                DECAL_ENTRY_RASTER_HASH,
                PropType::Key,
                vec![key(0x1111_1111), key(0x2222_2222), key(0x3333_3333)],
            ),
            array_property(
                DECAL_ENTRY_COLOR_HASHES[0],
                PropType::Vector4,
                vec![Value::Vector4([0.5, 0.0, 0.0, 0.0]); 3],
            ),
            array_property(
                DECAL_ENTRY_COLOR_HASHES[1],
                PropType::Vector4,
                vec![Value::Vector4([0.0, 0.5, 0.0, 0.0]); 3],
            ),
            array_property(
                DECAL_ENTRY_COLOR_HASHES[2],
                PropType::Vector4,
                vec![Value::Vector4([0.0, 0.0, 0.5, 0.0]); 3],
            ),
            array_property(
                DECAL_ENTRY_COLOR_HASHES[3],
                PropType::Vector4,
                vec![Value::Vector4([0.25, 0.25, 0.25, 0.0]); 3],
            ),
        ])
    }

    #[test]
    fn detects_decal_atlas_groups_by_low_halfword() {
        assert!(is_decal_dictionary_group(0xc67f_b185));
        assert!(is_decal_dictionary_group(0x1651));
        assert!(is_decal_dictionary_group(0xdead_1652));
        assert!(!is_decal_dictionary_group(0x0000_b104));
        assert!(!is_decal_dictionary_group(0xb185_0000));
    }

    #[test]
    fn assembles_entries_from_parallel_arrays() {
        let dictionary = DecalDictionary::from_file(&sample_file());
        assert_eq!(dictionary.entries.len(), 3);
        assert!(dictionary.uniform_arrays);
        assert_eq!(
            dictionary.material.map(|key| key.instance),
            Some(0xe539_0a98)
        );
        assert_eq!(dictionary.texture_size, Some([32.0, 32.0]));
        assert_eq!(dictionary.atlas_size, Some([512.0, 512.0]));

        assert_eq!(dictionary.entries[0].id.map(|key| key.instance), Some(0xb059_45fb));
        assert_eq!(dictionary.entries[1].id.map(|key| key.instance), Some(0x224a_5eba));
        assert_eq!(dictionary.entries[2].aspect_ratio, Some(1.5));
        assert_eq!(
            dictionary.entries[2].raster.map(|key| key.instance),
            Some(0x3333_3333)
        );
        assert!(dictionary.entries.iter().all(|entry| entry.colors_rgba8().is_some()));
    }

    #[test]
    fn longer_arrays_win_and_missing_slots_stay_none() {
        // ID 数组比其他数组长，条目数以最长数组为准。
        let mut values: Vec<Property> = sample_file()
            .values
            .into_iter()
            .filter(|property| property.hash != DECAL_ENTRY_ID_HASH)
            .collect();
        values.push(array_property(
            DECAL_ENTRY_ID_HASH,
            PropType::Key,
            vec![key(0x1), key(0x2), key(0x3), key(0x4), key(0x5)],
        ));
        let dictionary = DecalDictionary::from_file(&file_from(values));

        assert_eq!(dictionary.entries.len(), 5);
        assert!(!dictionary.uniform_arrays);
        assert_eq!(dictionary.entries[4].id.map(|key| key.instance), Some(0x5));
        // 其余数组只有 3 项，越界处为 None。
        assert_eq!(dictionary.entries[4].raster, None);
        assert_eq!(dictionary.entries[4].aspect_ratio, None);
        assert_eq!(dictionary.entries[4].colors[0], None);
    }

    #[test]
    fn absent_arrays_produce_no_entries() {
        let dictionary = DecalDictionary::from_file(&file_from(vec![scalar_property(
            DECAL_MATERIAL_HASH,
            PropType::Key,
            Value::Key(Key {
                instance: 0x1,
                type_id: 0x2f4e_681b,
                group: 0,
            }),
        )]));
        assert!(dictionary.entries.is_empty());
        assert!(dictionary.uniform_arrays);
        assert!(looks_like_dictionary(&file_from(vec![scalar_property(
            DECAL_MATERIAL_HASH,
            PropType::Key,
            Value::Key(Key {
                instance: 0x1,
                type_id: 0x2f4e_681b,
                group: 0,
            }),
        )])));
    }

    #[test]
    fn color_conversion_applies_srgb_encoding() {
        // 线性 0.5 → sRGB ≈ 0.7354 → 188。
        assert_eq!(linear_to_srgb_u8(0.5), 188);
        assert_eq!(linear_to_srgb_u8(0.0), 0);
        assert_eq!(linear_to_srgb_u8(1.0), 255);
        // 线性 0.25 → sRGB ≈ 0.5373 → 137。
        assert_eq!(linear_to_srgb_u8(0.25), 137);
        // 越界与非法值被钳制。
        assert_eq!(linear_to_srgb_u8(4.0), 255);
        assert_eq!(linear_to_srgb_u8(-1.0), 0);
        assert_eq!(linear_to_srgb_u8(f32::NAN), 0);

        // 存储值 0.25 = 线性 0.5，四色分别为 R/G/B/灰。
        let entry = DecalEntry {
            index: 0,
            id: None,
            raster: None,
            aspect_ratio: None,
            colors: [
                Some([0.25, 0.0, 0.0, 0.0]),
                Some([0.0, 0.25, 0.0, 0.0]),
                Some([0.0, 0.0, 0.25, 0.0]),
                Some([0.25, 0.25, 0.25, 0.0]),
            ],
        };
        assert_eq!(
            entry.colors_rgba8(),
            Some([[188, 0, 0, 255], [0, 188, 0, 255], [0, 0, 188, 255], [188, 188, 188, 255]])
        );
    }

    #[test]
    fn malformed_property_payload_is_rejected() {
        assert!(DecalDictionary::parse(&[0xff, 0xff, 0xff, 0xff]).is_err());
    }
}
