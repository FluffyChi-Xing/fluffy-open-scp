//! 区域可编辑字段写回（priorities P1-5 / map-workbench E2+E4）。
//!
//! - **水位**（E2）：区域 desc（instance [`REGION_DESC_INSTANCE`]）的
//!   [`HASH_WATER_LEVEL`]（Float，世界米）重编码写回；读侧换算见
//!   `region_map::region_water_level`（raw = (z+1024)×32）。
//! - **资源画刷清单**（E4）：`*EcoMapBrushes` property 的 stamp
//!   （[`HASH_BRUSH_STAMPS`]，Transform 数组，matrix[9]/[10] = 世界坐标，
//!   尾浮点 = 强度）增删。
//!
//! 两条链路都走 `PropertyFile::encode_canonical` 重编码 + 回读校验：
//! 除目标 hash 外，其余属性必须与原文逐项一致，否则整体放弃（与
//! `debug_tools::patch_property_refs` 的字节补丁同级别的安全约束；
//! 工具定义那种 Text 数组文件不适用本模块——那类文件请继续走字节补丁）。
use dbpf::{OverlayEntry, Package, ResourceId};

use crate::{Kind, PropertyFile, Property, Value};

/// property 资源类型（0x00B1B104）。
const PROPERTY_TYPE: u32 = 0x00B1_B104;
/// 区域 desc instance（每区域一条，group = 区域组）。
pub const REGION_DESC_INSTANCE: u32 = 0x51E7_A18D;
/// 水面世界高度（Float，米）。
pub const HASH_WATER_LEVEL: u32 = 0x0E16_BE1A;
/// 画刷清单名（String8，`*EcoMapBrushes`）。
pub const HASH_BRUSH_LIST_NAME: u32 = 0x00B2_CCCA;
/// 画刷 stamp 数组（Transform）。
pub const HASH_BRUSH_STAMPS: u32 = 0x02A9_07B6;
/// 画刷目标 map 名（String8，如 "coalEcoMap"）。
pub const HASH_BRUSH_MAP: u32 = 0x0DBA_3A9C;

/// 回读校验：重编码结果中，除 `targets` 外的全部属性须与原文逐项一致，
/// 且不得引入原文没有的属性。返回重解析结果。
fn verify_only_targets_changed(
    original: &PropertyFile,
    encoded: &[u8],
    targets: &[u32],
) -> Result<PropertyFile, String> {
    let reparsed = PropertyFile::parse(encoded).map_err(|e| format!("回读解析失败：{e}"))?;
    for p in &original.values {
        let Some(q) = reparsed.values.iter().find(|q| q.hash == p.hash) else {
            return Err(format!("属性 0x{:08X} 在重编码后丢失", p.hash));
        };
        if !targets.contains(&p.hash) && q != p {
            return Err(format!("属性 0x{:08X} 被意外改动", p.hash));
        }
    }
    if reparsed.values.iter().any(|q| !original.values.iter().any(|p| p.hash == q.hash)) {
        return Err("重编码引入了原文没有的属性".to_string());
    }
    Ok(reparsed)
}

/// 修改 property 文件中的标量 Float（或数组首元素），返回重编码字节。
pub fn patch_property_float(data: &[u8], hash: u32, value: f32) -> Result<Vec<u8>, String> {
    let file = PropertyFile::parse(data).map_err(|e| format!("解析失败：{e}"))?;
    let mut modified = file.clone();
    let Some(prop) = modified.values.iter_mut().find(|p| p.hash == hash) else {
        return Err(format!("属性 0x{hash:08X} 不存在"));
    };
    match &mut prop.kind {
        Kind::Scalar(v) if matches!(v, Value::Float(_)) => *v = Value::Float(value),
        Kind::Array(vs) if matches!(vs.first(), Some(Value::Float(_))) => {
            if let Some(Value::Float(first)) = vs.first_mut() {
                *first = value;
            }
        }
        _ => return Err(format!("属性 0x{hash:08X} 不是 Float 形态")),
    }
    let encoded = modified.encode_canonical().map_err(|e| format!("重编码失败：{e}"))?;
    verify_only_targets_changed(&file, &encoded, &[hash])?;
    Ok(encoded)
}

/// 画刷清单摘要（instance 供编辑命令定位）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct BrushList {
    pub instance: u32,
    /// 清单名（`coalEcoMapBrushes` 等）。
    pub name: String,
    /// 目标 map 名（0x0DBA3A9C），缺失为 None。
    pub map_name: Option<String>,
    /// stamp 世界坐标 (x, y)。
    pub stamps: Vec<(f32, f32)>,
}

/// 枚举某区域组的全部画刷清单 property。
pub fn brush_lists(package: &Package, group: u32) -> Result<Vec<BrushList>, String> {
    let mut out = Vec::new();
    for e in package.entries() {
        if e.id.type_id != PROPERTY_TYPE || e.id.group != group {
            continue;
        }
        let data = package.read(e).map_err(|err| err.to_string())?;
        let Ok(file) = PropertyFile::parse(&data) else {
            continue;
        };
        let Some(Property { kind: Kind::Scalar(Value::String8(name)), .. }) = file.get(HASH_BRUSH_LIST_NAME)
        else {
            continue;
        };
        if !(name.ends_with("brushes") || name.ends_with("Brushes")) || name == "brushes" {
            continue;
        }
        let mut stamps = Vec::new();
        if let Some(Property { kind: Kind::Array(vs), .. }) = file.get(HASH_BRUSH_STAMPS) {
            for v in vs {
                if let Value::Transform(t) = v {
                    if t.matrix.len() >= 11 {
                        stamps.push((t.matrix[9], t.matrix[10]));
                    }
                }
            }
        }
        let map_name = match file.get(HASH_BRUSH_MAP) {
            Some(Property { kind: Kind::Scalar(Value::String8(m)), .. }) => Some(m.clone()),
            _ => None,
        };
        out.push(BrushList {
            instance: e.id.instance,
            name: name.clone(),
            map_name,
            stamps,
        });
    }
    Ok(out)
}

/// 画刷 stamp 增删（对单条清单 property 重编码）。
///
/// - `add`：新 stamp 以清单内**首个现有 stamp 为模板**（仅改 matrix[9]/[10]
///   世界坐标；`strength` 提供且矩阵 ≥12 维时同步改末位强度）——克隆保证
///   flags/unknown/矩阵形状与游戏数据一致；清单为空时无法安全合成模板，报错。
/// - `remove`：按 [`BrushList::stamps`] 索引删除（越界即报错）。
///
/// 返回（重编码字节, 编辑后 stamp 数）。
pub fn edit_brush_stamps(
    data: &[u8],
    add: &[(f32, f32)],
    remove: &[usize],
    strength: Option<f32>,
) -> Result<(Vec<u8>, usize), String> {
    let file = PropertyFile::parse(data).map_err(|e| format!("解析失败：{e}"))?;
    let mut modified = file.clone();
    let Some(prop) = modified.values.iter_mut().find(|p| p.hash == HASH_BRUSH_STAMPS) else {
        return Err(format!("清单缺少 stamp 数组 0x{HASH_BRUSH_STAMPS:08X}"));
    };
    let Kind::Array(vs) = &mut prop.kind else {
        return Err("stamp 属性不是数组形态".to_string());
    };
    let mut idxs: Vec<usize> = remove.to_vec();
    idxs.sort_unstable();
    idxs.dedup();
    for &i in &idxs {
        if i >= vs.len() {
            return Err(format!("删除索引 {i} 越界（共 {} 条 stamp）", vs.len()));
        }
    }
    for &i in idxs.iter().rev() {
        vs.remove(i);
    }
    if !add.is_empty() {
        let template = vs
            .first()
            .cloned()
            .ok_or("清单为空，无法合成 stamp 模板（从零创建暂不支持）")?;
        let Value::Transform(template_t) = &template else {
            return Err("stamp 模板不是 Transform".to_string());
        };
        for &(x, y) in add {
            let mut t = template_t.clone();
            if t.matrix.len() < 11 {
                return Err(format!("模板矩阵维度 {} 不足", t.matrix.len()));
            }
            t.matrix[9] = x;
            t.matrix[10] = y;
            if let Some(s) = strength {
                if t.matrix.len() >= 12 {
                    let n = t.matrix.len();
                    t.matrix[n - 1] = s;
                }
            }
            vs.push(Value::Transform(t));
        }
    }
    let encoded = modified.encode_canonical().map_err(|e| format!("重编码失败：{e}"))?;
    let reparsed = verify_only_targets_changed(&file, &encoded, &[HASH_BRUSH_STAMPS])?;
    let count = match reparsed.get(HASH_BRUSH_STAMPS) {
        Some(Property { kind: Kind::Array(vs), .. }) => vs.len(),
        _ => 0,
    };
    Ok((encoded, count))
}

/// 组装单条目 overlay 包字节（供命令层落盘）。
pub fn single_entry_overlay(instance: u32, group: u32, data: Vec<u8>) -> Result<Vec<u8>, String> {
    dbpf::write_uncompressed_overlay(&[OverlayEntry::new(
        ResourceId {
            type_id: PROPERTY_TYPE,
            group,
            instance,
        },
        data,
    )])
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PropType, PropertyEncoding, Transform};

    fn transform_stamp(x: f32, y: f32, strength: f32) -> Value {
        Value::Transform(Transform {
            flags: 15,
            unknown: Some(0.0),
            matrix: vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, x, y, strength],
        })
    }

    /// 合成画刷清单 + 水位 Float + 无关 String8，encode 后作为被测字节。
    fn fixture() -> (Vec<u8>, u32) {
        let file = PropertyFile {
            values: vec![
                Property {
                    hash: HASH_BRUSH_LIST_NAME,
                    prop_type: PropType::String8,
                    kind: Kind::Scalar(Value::String8("coalEcoMapBrushes".to_string())),
                    encoding: PropertyEncoding { flags: 0, array_item_size: None },
                },
                Property {
                    hash: HASH_BRUSH_STAMPS,
                    prop_type: PropType::Transform,
                    kind: Kind::Array(vec![transform_stamp(100.0, 200.0, 1.0), transform_stamp(-300.0, 400.0, 0.5)]),
                    encoding: PropertyEncoding { flags: 0x30, array_item_size: Some(54) },
                },
                Property {
                    hash: HASH_WATER_LEVEL,
                    prop_type: PropType::Float,
                    kind: Kind::Scalar(Value::Float(-870.0)),
                    encoding: PropertyEncoding { flags: 0, array_item_size: None },
                },
                Property {
                    hash: 0x0AAA_0001,
                    prop_type: PropType::String8,
                    kind: Kind::Scalar(Value::String8("untouched".to_string())),
                    encoding: PropertyEncoding { flags: 0, array_item_size: None },
                },
            ],
            claimed_count: 4,
        };
        (file.encode_canonical().unwrap(), 0xDEAD_BEEF)
    }

    #[test]
    fn water_level_patch_changes_only_target() {
        let (data, group) = fixture();
        let patched = patch_property_float(&data, HASH_WATER_LEVEL, -860.5).unwrap();
        let file = PropertyFile::parse(&patched).unwrap();
        assert_eq!(
            file.get(HASH_WATER_LEVEL),
            Some(&Property {
                hash: HASH_WATER_LEVEL,
                prop_type: PropType::Float,
                kind: Kind::Scalar(Value::Float(-860.5)),
                encoding: PropertyEncoding { flags: 0, array_item_size: None },
            })
        );
        // 无关属性逐项一致
        let original = PropertyFile::parse(&data).unwrap();
        for p in &original.values {
            if p.hash == HASH_WATER_LEVEL {
                continue;
            }
            assert_eq!(file.get(p.hash), Some(p));
        }
        let _ = group;
    }

    #[test]
    fn brush_stamp_add_remove_roundtrip() {
        let (data, _) = fixture();
        // 加一枚（强度 0.8）
        let (patched, count) =
            edit_brush_stamps(&data, &[(555.5, -666.25)], &[], Some(0.8)).unwrap();
        assert_eq!(count, 3);
        let file = PropertyFile::parse(&patched).unwrap();
        let Some(Property { kind: Kind::Array(vs), .. }) = file.get(HASH_BRUSH_STAMPS) else {
            panic!("stamp 数组丢失");
        };
        let Value::Transform(t) = vs.last().unwrap() else { panic!("非 Transform") };
        assert_eq!((t.matrix[9], t.matrix[10]), (555.5, -666.25));
        assert_eq!(t.matrix[11], 0.8);
        assert_eq!(t.flags, 15, "模板 flags 必须保留");
        // 删两枚（原第 0、1 枚）
        let (patched2, count2) = edit_brush_stamps(&patched, &[], &[0, 1], None).unwrap();
        assert_eq!(count2, 1);
        let file2 = PropertyFile::parse(&patched2).unwrap();
        let Some(Property { kind: Kind::Array(vs2), .. }) = file2.get(HASH_BRUSH_STAMPS) else {
            panic!("stamp 数组丢失");
        };
        let Value::Transform(t2) = vs2.first().unwrap() else { panic!("非 Transform") };
        assert_eq!((t2.matrix[9], t2.matrix[10]), (555.5, -666.25));
    }

    #[test]
    fn brush_remove_out_of_bounds_errors() {
        let (data, _) = fixture();
        let err = edit_brush_stamps(&data, &[], &[5], None).unwrap_err();
        assert!(err.contains("越界"), "实际：{err}");
    }
}
