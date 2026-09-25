//! 区域可编辑字段写回（priorities P1-5 / map-workbench E2+E4）。
//!
//! - **水位**（E2）：区域 desc（instance [`REGION_DESC_INSTANCE`]）的
//!   [`HASH_WATER_LEVEL`]（Float，世界米）写回；读侧换算见
//!   `region_map::region_water_level`（raw = (z+1024)×32）。
//! - **资源画刷清单**（E4）：`*EcoMapBrushes` property 的 stamp
//!   （[`HASH_BRUSH_STAMPS`]，Transform 数组，matrix[9]/[10] = 世界坐标，
//!   尾浮点 = 强度）增删。
//!
//! **两条链路都是字节级补丁**：结构遍历定位载荷偏移，只改目标字节，
//! 其余与原文件逐字节一致。2026-09-26 实测 encode_canonical 重编码的区域
//! desc overlay 会让游戏无法选择区域/预览（重编码引入游戏不接受的字节差异，
//! 与 debug_tools 探针记录的 Text 数组问题同类），故全部改走字节补丁。
//! 校验双保险：①回读解析——除目标 hash 外逐属性一致；②水位补丁的字节
//! 差异必须恰好 4 字节。
use dbpf::{OverlayEntry, Package, ResourceId};

use crate::{Kind, PropType, PropertyFile, Property, Value};

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

// ── 字节级结构遍历 ──
//
// 复刻 PropertyFile::parse 的游走逻辑（含 String8 的 0x100 短长度怪癖与
// Transform 的 flags→矩阵维数），但不构建值——只为拿到载荷的精确偏移。

/// 固定尺寸类型的载荷字节数（与 `fixed_value_size` 对齐）。
fn fixed_payload_size(prop_type: PropType) -> Option<usize> {
    Some(match prop_type {
        PropType::Bool => 1,
        PropType::Int32 | PropType::UInt32 | PropType::Float => 4,
        PropType::Key => 12,
        PropType::Text => 8,
        PropType::Vector2 => 8,
        PropType::Vector3 | PropType::ColorRgb => 12,
        PropType::Vector4 | PropType::ColorRgba => 16,
        PropType::BoundingBox => 24,
        PropType::Transform | PropType::String8 | PropType::String16 => return None,
    })
}

/// 变长值（String8/String16/Transform）在 `pos` 处的载荷字节数。
/// `flags` 为条目标志（仅 String8 的 0x100 短长度参与）；数组元素按 flags=0 读。
fn variable_payload_size(data: &[u8], pos: usize, prop_type: PropType, flags: u16) -> Result<usize, String> {
    let need = |end: usize| -> Result<(), String> {
        if end > data.len() {
            Err(format!("属性文件在 0x{pos:X} 处截断"))
        } else {
            Ok(())
        }
    };
    match prop_type {
        PropType::String8 => {
            need(pos + 4)?;
            let length = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
            if length == 0 && flags & 0x100 != 0 {
                need(pos + 5)?;
                Ok(4 + 1 + data[pos + 4] as usize)
            } else {
                need(pos + 4 + length)?;
                Ok(4 + length)
            }
        }
        PropType::String16 => {
            need(pos + 4)?;
            let count = i32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            if count < 0 {
                return Err("String16 count 为负".to_string());
            }
            need(pos + 4 + count as usize * 2)?;
            Ok(4 + count as usize * 2)
        }
        PropType::Transform => {
            need(pos + 2)?;
            let tflags = u16::from_be_bytes([data[pos], data[pos + 1]]);
            let matrix = match tflags {
                0x0C => 3,
                0x0D => 4,
                _ => 12,
            };
            let size = 2 + if tflags == 15 { 4 } else { 0 } + matrix * 4;
            need(pos + size)?;
            Ok(size)
        }
        _ => Err(format!("类型 {prop_type:?} 不是变长类型")),
    }
}

/// 单条 property 在文件内的位置。
struct PropertyLoc {
    hash: u32,
    prop_type: PropType,
    flags: u16,
    /// 标量：载荷偏移。数组：count 字段偏移。
    payload_offset: usize,
    is_array: bool,
    /// 数组：各元素的 (偏移, 字节数)。
    items: Vec<(usize, usize)>,
}

/// 结构遍历：定位 `hash` 对应的条目。
fn locate_property(data: &[u8], hash: u32) -> Result<PropertyLoc, String> {
    if data.len() < 4 {
        return Err("属性文件过短".to_string());
    }
    let claimed = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut pos = 4usize;
    let mut parsed = 0usize;
    while parsed < claimed && pos < data.len() {
        if pos + 8 > data.len() {
            return Err("属性头截断".to_string());
        }
        let entry_hash = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        let file_type = u16::from_be_bytes([data[pos + 4], data[pos + 5]]);
        let flags = u16::from_be_bytes([data[pos + 6], data[pos + 7]]);
        let Some(prop_type) = PropType::from_file_type(file_type) else {
            return Err(format!("0x{entry_hash:08X} 未知类型 0x{file_type:04X}"));
        };
        pos += 8;
        let payload_offset = pos;
        let (is_array, items) = if flags & 0x30 == 0 {
            // 标量
            let size = match fixed_payload_size(prop_type) {
                Some(n) => n,
                None => variable_payload_size(data, pos, prop_type, flags)?,
            };
            if payload_offset + size > data.len() {
                return Err("属性载荷截断".to_string());
            }
            pos += size;
            (false, Vec::new())
        } else if flags & 0x40 == 0 {
            // 数组：count i32 + item_size i32 + 逐元素
            if pos + 8 > data.len() {
                return Err("数组头截断".to_string());
            }
            let count = i32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
            pos += 8;
            if count < 0 {
                return Err(format!("0x{entry_hash:08X} 数组 count 为负"));
            }
            let mut items = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let start = pos;
                let size = match fixed_payload_size(prop_type) {
                    Some(n) => n,
                    None => variable_payload_size(data, pos, prop_type, 0)?,
                };
                if start + size > data.len() {
                    return Err("数组元素截断".to_string());
                }
                items.push((start, size));
                pos += size;
            }
            (true, items)
        } else {
            // 空变体：无载荷
            (false, Vec::new())
        };
        parsed += 1;
        if entry_hash == hash {
            return Ok(PropertyLoc {
                hash,
                prop_type,
                flags,
                payload_offset,
                is_array,
                items,
            });
        }
    }
    Err(format!("属性 0x{hash:08X} 不存在"))
}

/// 水位补丁的不变量：文件长度不变，且全部字节差异落在目标 4 字节载荷内
/// （值变化不要求 4 字节全变，例如 -870→-860.5 只差 2 字节）。
fn assert_diff_within(old: &[u8], new: &[u8], payload: std::ops::Range<usize>) -> Result<(), String> {
    if old.len() != new.len() {
        return Err(format!("补丁改变了文件长度 {} → {}", old.len(), new.len()));
    }
    for (i, (a, b)) in old.iter().zip(new.iter()).enumerate() {
        if a != b && !payload.contains(&i) {
            return Err(format!("字节 0x{i:X} 在目标载荷之外被改动，放弃写出"));
        }
    }
    Ok(())
}

/// 语义校验：重解析后除 `targets` 外全部属性与原文一致。
fn verify_only_targets_changed(
    original: &PropertyFile,
    patched: &[u8],
    targets: &[u32],
) -> Result<PropertyFile, String> {
    let reparsed = PropertyFile::parse(patched).map_err(|e| format!("回读解析失败：{e}"))?;
    for p in &original.values {
        let Some(q) = reparsed.values.iter().find(|q| q.hash == p.hash) else {
            return Err(format!("属性 0x{:08X} 在补丁后丢失", p.hash));
        };
        if !targets.contains(&p.hash) && q != p {
            return Err(format!("属性 0x{:08X} 被意外改动", p.hash));
        }
    }
    if reparsed
        .values
        .iter()
        .any(|q| !original.values.iter().any(|p| p.hash == q.hash))
    {
        return Err("补丁引入了原文没有的属性".to_string());
    }
    Ok(reparsed)
}

/// 修改 property 文件中的标量 Float（字节级）：定位载荷偏移后仅改 4 字节
/// （BE f32），其余与原文件逐字节一致。返回补丁后的字节。
pub fn patch_property_float(data: &[u8], hash: u32, value: f32) -> Result<Vec<u8>, String> {
    let loc = locate_property(data, hash)?;
    if loc.is_array || loc.prop_type != PropType::Float {
        return Err(format!("属性 0x{hash:08X} 不是 Float 标量"));
    }
    let original = PropertyFile::parse(data).map_err(|e| format!("解析失败：{e}"))?;
    let mut out = data.to_vec();
    out[loc.payload_offset..loc.payload_offset + 4].copy_from_slice(&value.to_be_bytes());
    assert_diff_within(data, &out, loc.payload_offset..loc.payload_offset + 4)?;
    verify_only_targets_changed(&original, &out, &[hash])?;
    Ok(out)
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

/// 画刷 stamp 增删（字节级）：定位 stamp 数组，删除按元素字节区间摘除，
/// 新增以**首个保留元素的原始字节**为模板（仅就地改写 matrix[9]/[10] 世界
/// 坐标 BE f32；`strength` 提供且矩阵 ≥12 维时改写末位强度）——flags/
/// unknown/元素尺寸与游戏数据保持逐字节一致。最后改写 count 字段。
///
/// - `remove`：按 [`BrushList::stamps`] 索引删除（越界即报错）。
/// - 清单为空（无可保留模板）时新增报错：从零合成 stamp 字节暂不支持。
///
/// 返回（补丁后字节, 编辑后 stamp 数）。
pub fn edit_brush_stamps(
    data: &[u8],
    add: &[(f32, f32)],
    remove: &[usize],
    strength: Option<f32>,
) -> Result<(Vec<u8>, usize), String> {
    let original = PropertyFile::parse(data).map_err(|e| format!("解析失败：{e}"))?;
    let loc = locate_property(data, HASH_BRUSH_STAMPS)?;
    if !loc.is_array || loc.prop_type != PropType::Transform {
        return Err(format!(
            "属性 0x{HASH_BRUSH_STAMPS:08X} 不是 Transform 数组"
        ));
    }
    let mut idxs: Vec<usize> = remove.to_vec();
    idxs.sort_unstable();
    idxs.dedup();
    for &i in &idxs {
        if i >= loc.items.len() {
            return Err(format!("删除索引 {i} 越界（共 {} 条 stamp）", loc.items.len()));
        }
    }
    // 保留集合（未删除的元素字节区间）
    let kept: Vec<(usize, usize)> = loc
        .items
        .iter()
        .enumerate()
        .filter(|(i, _)| !idxs.contains(i))
        .map(|(_, range)| *range)
        .collect();
    // 新增元素的字节：克隆首个保留元素的原始字节，就地改写矩阵分量
    let mut added_bytes: Vec<Vec<u8>> = Vec::new();
    if !add.is_empty() {
        let Some(&(toff, tlen)) = kept.first() else {
            return Err("清单为空（无可保留模板），从零合成 stamp 暂不支持".to_string());
        };
        let tflags = u16::from_be_bytes([data[toff], data[toff + 1]]);
        let matrix_len = match tflags {
            0x0C => 3,
            0x0D => 4,
            _ => 12,
        };
        if matrix_len < 11 {
            return Err(format!("模板矩阵维度 {matrix_len} 不足"));
        }
        let matrix_base = toff + 2 + if tflags == 15 { 4 } else { 0 };
        for &(x, y) in add {
            let mut item = data[toff..toff + tlen].to_vec();
            let write_f32 = |bytes: &mut [u8], index: usize, v: f32| {
                let at = matrix_base - toff + index * 4;
                bytes[at..at + 4].copy_from_slice(&v.to_be_bytes());
            };
            write_f32(&mut item, 9, x);
            write_f32(&mut item, 10, y);
            if let Some(s) = strength {
                if matrix_len >= 12 {
                    write_f32(&mut item, matrix_len - 1, s);
                }
            }
            added_bytes.push(item);
        }
    }
    // 组装：①按原始偏移从高到低摘除被删元素字节；②在数组载荷新末尾插入
    // 新增字节；③改写 count。数组元素连续，故新载荷末尾 = 原末尾 − 摘除字节。
    let mut out = data.to_vec();
    let mut removals: Vec<(usize, usize)> = loc
        .items
        .iter()
        .enumerate()
        .filter(|(i, _)| idxs.contains(i))
        .map(|(i, _)| loc.items[i])
        .collect();
    removals.sort_unstable();
    for &(off, len) in removals.iter().rev() {
        out.splice(off..off + len, std::iter::empty());
    }
    let removed_bytes: usize = removals.iter().map(|&(_, len)| len).sum();
    if !added_bytes.is_empty() {
        let original_payload_end = loc
            .items
            .last()
            .map(|&(off, len)| off + len)
            .expect("有新增时必有元素");
        let insert_at = original_payload_end - removed_bytes;
        let mut inserted = Vec::new();
        for item in &added_bytes {
            inserted.extend_from_slice(item);
        }
        out.splice(insert_at..insert_at, inserted);
    }
    // count 字段在 payload_offset（摘除/插入都不影响它之前的字节）
    let new_count = (kept.len() + added_bytes.len()) as i32;
    out[loc.payload_offset..loc.payload_offset + 4].copy_from_slice(&new_count.to_be_bytes());

    // 校验：语义（除 stamp 数组外逐属性一致）+ stamp 值符合预期
    let reparsed = verify_only_targets_changed(&original, &out, &[HASH_BRUSH_STAMPS])?;
    let Some(Property { kind: Kind::Array(vs), .. }) = reparsed.get(HASH_BRUSH_STAMPS) else {
        return Err("补丁后 stamp 数组丢失".to_string());
    };
    if vs.len() != kept.len() + added_bytes.len() {
        return Err(format!("补丁后 stamp 数 {} ≠ 预期 {}", vs.len(), kept.len() + added_bytes.len()));
    }
    Ok((out, vs.len()))
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
