//! Debug 工具解锁 overlay（`enable_debug_tools` 探针的产品化，priorities P0-2）。
//!
//! 机制：三个开发期工具（Edit Unit Prop Tool / Edit Snap Points Tool /
//! Unit Connecter Tool）是被 property 门控关掉的——把菜单分类键
//! [`HASH_TOOL_CATEGORY`]（debug 值 0x3D752D6A）与 UI 子分类键
//! [`HASH_UI_TOOL_CATEGORY`]（debug 值 0xA2A33338 "pipetinetools"）重指到
//! 可见建筑分类即可重新打开；其原父对象 0xBE5744E0 是 0 属性空壳，重指到
//! 可见建筑工具（310 条）共用的真实父对象 [`LIVE_TOOL_PARENT`]。
//! 其余字段（标题/描述/图标/位置）原样保留——采用逐值字节补丁而非整份重编码，
//! 保证除目标字段外与零售包逐字节相同。
use dbpf::{OverlayEntry, Package};

use crate::{Kind, PropertyFile, Value};

/// property 资源类型。
const PROPERTY_TYPE: u32 = 0x00B1_B104;
/// 工具定义所在组（可见建筑工具 310 条同组）。
pub const TOOL_GROUP: u32 = 0x0987_8A01;
/// 菜单分类键。
pub const HASH_TOOL_CATEGORY: u32 = 0x0975_695E;
/// UI 子分类键。
pub const HASH_UI_TOOL_CATEGORY: u32 = 0x0DB9_FC63;
/// 工具父对象键。
pub const HASH_PARENT: u32 = 0x00B2_CCCB;
/// 可见建筑工具共同的真实父对象。
pub const LIVE_TOOL_PARENT: u32 = 0xAF04_2E9A;
/// 默认目标菜单分类（建筑工具可见分类，探针默认值）。
pub const DEFAULT_CATEGORY: u32 = 0xC710_B6E9;
/// 默认 UI 子分类（探针默认值）。
pub const DEFAULT_UI_CATEGORY: u32 = 0x9D2E_F585;
/// 三个被门控的 debug 工具 instance。
pub const DEBUG_TOOLS: [u32; 3] = [0x53B7_7423, 0xA2AA_4268, 0xB65E_1F2E];

/// 单个工具的补丁摘要。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DebugToolPatch {
    pub instance: u32,
    pub old_categories: Vec<u32>,
    pub new_category: u32,
    pub old_ui_categories: Vec<u32>,
    pub new_ui_category: u32,
    pub old_parents: Vec<u32>,
    pub new_parent: u32,
    /// 补丁后条目字节数。
    pub size_bytes: usize,
}

/// 生成解锁 overlay 的全部条目（不落盘；落盘用
/// `dbpf::write_uncompressed_overlay`）。
///
/// 任一工具缺失或补丁校验失败都会整体报错——产品命令要求确定性，
/// 不做探针时代的"跳过继续"。
pub fn build_debug_tools_overlay(
    package: &Package,
    category: u32,
    ui_category: u32,
) -> Result<(Vec<OverlayEntry>, Vec<DebugToolPatch>), String> {
    let mut entries = Vec::new();
    let mut patches = Vec::new();
    for target in DEBUG_TOOLS {
        let Some(entry) = package.entries().iter().find(|e| {
            e.id.type_id == PROPERTY_TYPE && e.id.group == TOOL_GROUP && e.id.instance == target
        }) else {
            return Err(format!(
                "未找到工具 0x{target:08X}（group 0x{TOOL_GROUP:08X}）——基础包不含工具定义"
            ));
        };
        let data = package.read(entry).map_err(|e| e.to_string())?;
        let file =
            PropertyFile::parse(&data).map_err(|e| format!("0x{target:08X} 解析失败：{e}"))?;
        let old_categories = key_instances(&file, HASH_TOOL_CATEGORY);
        let old_ui_categories = key_instances(&file, HASH_UI_TOOL_CATEGORY);
        let old_parents = key_instances(&file, HASH_PARENT);
        // 字段可能有多个 Key（如 0xB65E1F2E 的 UI 分类是两个），全部改写
        let refs: Vec<(u32, u32, u32)> = old_categories
            .iter()
            .map(|old| (HASH_TOOL_CATEGORY, *old, category))
            .chain(
                old_ui_categories
                    .iter()
                    .map(|old| (HASH_UI_TOOL_CATEGORY, *old, ui_category)),
            )
            .chain(
                old_parents
                    .iter()
                    .map(|old| (HASH_PARENT, *old, LIVE_TOOL_PARENT)),
            )
            .collect();
        let patched = patch_property_refs(&data, &refs, |f, h| key_instances(f, h))?;
        patches.push(DebugToolPatch {
            instance: target,
            old_categories,
            new_category: category,
            old_ui_categories,
            new_ui_category: ui_category,
            old_parents,
            new_parent: LIVE_TOOL_PARENT,
            size_bytes: patched.len(),
        });
        entries.push(OverlayEntry::new(entry.id, patched));
    }
    Ok((entries, patches))
}

/// 逐值字节补丁：把 `(hash, 旧值, 新值)` 里的引用值就地改写（`extract` 决定取
/// Key instance 还是 Text 串 id）。
///
/// **不重编码整份属性**——`encode_canonical` 会对保留的数组元数据做严格校验
/// （工具定义含 Text 数组，实测报 `InvalidArrayItemSize`），且整包重编码会引入
/// 无关字节差异。就地补丁只动 4 字节，其余与零售包逐字节相同。
///
/// 安全约束：旧值必须在该负载里**恰好出现一次**；补丁后重新解析，要求
/// 「目标 hash 的新值正确」且「其余属性与原文完全一致」，否则整体放弃。
pub fn patch_property_refs<F>(
    data: &[u8],
    patches: &[(u32, u32, u32)],
    extract: F,
) -> Result<Vec<u8>, String>
where
    F: Fn(&PropertyFile, u32) -> Vec<u32>,
{
    let original = PropertyFile::parse(data).map_err(|e| format!("原始解析失败 {e}"))?;
    let mut out = data.to_vec();
    let mut applied: Vec<(u32, u32, u32, usize)> = Vec::new();
    for (hash, old, new) in patches {
        // 属性负载里 hash / Key instance 都是**大端**存储（原包实证：
        // 0x3D752D6A 以 `3d 75 2d 6a` 出现），故先试 BE 再退回 LE。
        let hits: Vec<usize> = [old.to_be_bytes(), old.to_le_bytes()]
            .into_iter()
            .find_map(|needle| {
                let found: Vec<usize> = out
                    .windows(4)
                    .enumerate()
                    .filter(|(_, w)| **w == needle)
                    .map(|(i, _)| i)
                    .collect();
                // 恰好多于 0 次时采用该字节序，便于报出「多次命中」的错误
                (!found.is_empty()).then_some(found)
            })
            .unwrap_or_default();
        if hits.len() != 1 {
            return Err(format!(
                "0x{hash:08X} 旧值 0x{old:08X} 在负载中出现 {} 次（要求恰好 1 次）",
                hits.len()
            ));
        }
        out[hits[0]..hits[0] + 4].copy_from_slice(&new.to_be_bytes());
        applied.push((*hash, *old, *new, hits[0]));
    }

    let patched = PropertyFile::parse(&out).map_err(|e| format!("补丁后解析失败 {e}"))?;
    for (hash, old, new, at) in &applied {
        // 该 hash 下的**全部** Key 都必须已变为新值（含数组多元素的情形）
        let remaining = extract(&patched, *hash);
        if remaining.iter().any(|v| v != new) {
            return Err(format!(
                "@0x{at:X} 0x{hash:08X} 期望整体变为 0x{new:08X}（旧值 0x{old:08X}），实际 {:?}",
                hex_list(&remaining)
            ));
        }
    }
    // 除被改写的 hash 外，其余属性必须逐项一致
    let edited: Vec<u32> = applied.iter().map(|(h, ..)| *h).collect();
    for property in &original.values {
        if edited.contains(&property.hash) {
            continue;
        }
        let Some(other) = patched.values.iter().find(|p| p.hash == property.hash) else {
            return Err(format!("属性 0x{:08X} 在补丁后丢失", property.hash));
        };
        if other != property {
            return Err(format!("属性 0x{:08X} 被意外改动", property.hash));
        }
    }
    Ok(out)
}

/// 按 Property hash 精确定位 Key 字段，只改该字段中 instance 等于 `old` 的
/// 项。用于诊断 action 映射，避免在整个负载里扫描同一个 u32 时误改图标或
/// 其他属性中的相同数值。
pub fn patch_key_property_instances(
    data: &[u8],
    property_hash: u32,
    old: u32,
    new: u32,
) -> Result<Vec<u8>, String> {
    let file = PropertyFile::parse(data).map_err(|e| format!("原始解析失败 {e}"))?;
    let values = key_instances(&file, property_hash);
    if !values.contains(&old) {
        return Err(format!(
            "属性 0x{property_hash:08X} 没有 Key instance 0x{old:08X}"
        ));
    }
    let mut out = data.to_vec();
    let hash_bytes = property_hash.to_be_bytes();
    let mut starts = Vec::new();
    for at in out
        .windows(4)
        .enumerate()
        .filter_map(|(i, w)| (w == hash_bytes).then_some(i))
    {
        if at + 8 > out.len() {
            continue;
        }
        let ty = u16::from_be_bytes([out[at + 4], out[at + 5]]);
        let flags = u16::from_be_bytes([out[at + 6], out[at + 7]]);
        if ty != 32 || (flags & 0x40) != 0 {
            continue;
        }
        if flags & 0x30 == 0 {
            starts.push(at + 8);
        } else {
            if at + 16 > out.len() {
                continue;
            }
            let count = i32::from_be_bytes(out[at + 8..at + 12].try_into().unwrap());
            let item_size = i32::from_be_bytes(out[at + 12..at + 16].try_into().unwrap());
            if count < 0 || item_size < 12 {
                continue;
            }
            for i in 0..count as usize {
                let Some(pos) = (at + 16).checked_add(i.saturating_mul(item_size as usize)) else {
                    continue;
                };
                if pos + 4 <= out.len() {
                    starts.push(pos);
                }
            }
        }
    }
    let mut changed = 0;
    for pos in starts {
        if out[pos..pos + 4] == old.to_be_bytes() {
            out[pos..pos + 4].copy_from_slice(&new.to_be_bytes());
            changed += 1;
        }
    }
    if changed == 0 {
        return Err(format!(
            "属性 0x{property_hash:08X} 的序列化 Key 没有命中 0x{old:08X}"
        ));
    }
    let patched = PropertyFile::parse(&out).map_err(|e| format!("补丁后解析失败 {e}"))?;
    let remaining = key_instances(&patched, property_hash);
    if remaining.iter().any(|v| *v == old) || !remaining.iter().any(|v| *v == new) {
        return Err(format!(
            "属性 0x{property_hash:08X} 补丁校验失败，实际 {:?}",
            hex_list(&remaining)
        ));
    }
    Ok(out)
}

/// 按 Property hash 精确修改标量数值。菜单诊断包用它设置同级菜单位置，
/// 防止负载中其他字段恰好也出现相同的数值而被误改。
pub fn patch_property_i32(
    data: &[u8],
    property_hash: u32,
    old: i32,
    new: i32,
) -> Result<Vec<u8>, String> {
    let file = PropertyFile::parse(data).map_err(|e| format!("原始解析失败 {e}"))?;
    let Some(property) = file.get(property_hash) else {
        return Err(format!("缺少 Int32 属性 0x{property_hash:08X}"));
    };
    let (current, number_kind) = match &property.kind {
        Kind::Scalar(Value::Int32(value)) => (*value as f32, 9u16),
        Kind::Scalar(Value::UInt32(value)) => (*value as f32, 10u16),
        Kind::Scalar(Value::Float(value)) => (*value, 13u16),
        Kind::Array(values) => match values.first() {
            Some(Value::Int32(value)) => (*value as f32, 9u16),
            Some(Value::UInt32(value)) => (*value as f32, 10u16),
            Some(Value::Float(value)) => (*value, 13u16),
            _ => return Err(format!("属性 0x{property_hash:08X} 不是数值数组")),
        },
        _ => return Err(format!("属性 0x{property_hash:08X} 不是数值")),
    };
    if current != old as f32 {
        return Err(format!(
            "属性 0x{property_hash:08X} 当前为 {current}，预期 {old}"
        ));
    }
    let needle = property_hash.to_be_bytes();
    let mut out = data.to_vec();
    let mut changed = false;
    let property_offsets: Vec<usize> = out
        .windows(4)
        .enumerate()
        .filter_map(|(i, w)| (w == needle).then_some(i))
        .collect();
    for at in property_offsets {
        if at + 12 > out.len() {
            continue;
        }
        let ty = u16::from_be_bytes([out[at + 4], out[at + 5]]);
        let flags = u16::from_be_bytes([out[at + 6], out[at + 7]]);
        let positions: Vec<usize> = if flags & 0x30 == 0 {
            vec![at + 8]
        } else {
            if at + 16 > out.len() {
                continue;
            }
            let count = i32::from_be_bytes(out[at + 8..at + 12].try_into().unwrap());
            let item_size = i32::from_be_bytes(out[at + 12..at + 16].try_into().unwrap());
            if count <= 0 || item_size < 4 {
                continue;
            }
            (0..count as usize)
                .filter_map(|i| at.checked_add(16 + i.saturating_mul(item_size as usize)))
                .collect()
        };
        for value_at in positions {
            if value_at + 4 > out.len() {
                continue;
            }
            let matches = match number_kind {
                9 => {
                    ty == 9
                        && i32::from_be_bytes(out[value_at..value_at + 4].try_into().unwrap())
                            == old
                }
                10 => {
                    ty == 10
                        && u32::from_be_bytes(out[value_at..value_at + 4].try_into().unwrap())
                            == old as u32
                }
                13 => {
                    ty == 13
                        && f32::from_be_bytes(out[value_at..value_at + 4].try_into().unwrap())
                            == old as f32
                }
                _ => false,
            };
            if matches {
                let bytes = if number_kind == 13 {
                    (new as f32).to_be_bytes()
                } else {
                    new.to_be_bytes()
                };
                out[value_at..value_at + 4].copy_from_slice(&bytes);
                changed = true;
            }
        }
        if changed {
            break;
        }
    }
    if !changed {
        return Err(format!(
            "属性 0x{property_hash:08X} 的序列化数值未命中 {old}"
        ));
    }
    let patched = PropertyFile::parse(&out).map_err(|e| format!("补丁后解析失败 {e}"))?;
    match patched.get(property_hash).map(|p| &p.kind) {
        Some(Kind::Scalar(Value::Int32(value))) if *value == new => Ok(out),
        Some(Kind::Scalar(Value::UInt32(value))) if *value == new as u32 => Ok(out),
        Some(Kind::Scalar(Value::Float(value))) if *value == new as f32 => Ok(out),
        Some(Kind::Array(values))
            if values.first().is_some_and(|value| match value {
                Value::Int32(v) => *v == new,
                Value::UInt32(v) => *v == new as u32,
                Value::Float(v) => *v == new as f32,
                _ => false,
            }) =>
        {
            Ok(out)
        }
        actual => Err(format!(
            "属性 0x{property_hash:08X} 补丁校验失败：{actual:?}"
        )),
    }
}

/// 按 Property hash 精确修改标量 bool。环境 Terrain Authoring 开关使用此
/// 字段；就地修改只改变 1 个值字节，保留原包的字段顺序和编码。
pub fn patch_property_bool(
    data: &[u8],
    property_hash: u32,
    old: bool,
    new: bool,
) -> Result<Vec<u8>, String> {
    let file = PropertyFile::parse(data).map_err(|e| format!("原始解析失败 {e}"))?;
    let Some(property) = file.get(property_hash) else {
        return Err(format!("缺少 bool 属性 0x{property_hash:08X}"));
    };
    match &property.kind {
        Kind::Scalar(Value::Bool(value)) if *value == old => {}
        Kind::Scalar(Value::Bool(value)) => {
            return Err(format!(
                "属性 0x{property_hash:08X} 当前为 {value}，预期 {old}"
            ));
        }
        _ => return Err(format!("属性 0x{property_hash:08X} 不是标量 bool")),
    }

    let needle = property_hash.to_be_bytes();
    let mut out = data.to_vec();
    let hits: Vec<usize> = out
        .windows(4)
        .enumerate()
        .filter_map(|(i, w)| (w == needle).then_some(i))
        .collect();
    if hits.len() != 1 {
        return Err(format!(
            "属性 0x{property_hash:08X} 的序列化 hash 命中 {} 次，拒绝修改",
            hits.len()
        ));
    }
    let at = hits[0];
    if at + 9 > out.len()
        || u16::from_be_bytes([out[at + 4], out[at + 5]]) != 1
        || u16::from_be_bytes([out[at + 6], out[at + 7]]) & 0x30 != 0
    {
        return Err(format!(
            "属性 0x{property_hash:08X} 的序列化头不是标量 bool (ty={:04X}, flags={:04X})",
            u16::from_be_bytes([out[at + 4], out[at + 5]]),
            u16::from_be_bytes([out[at + 6], out[at + 7]])
        ));
    }
    if out[at + 8] != u8::from(old) {
        return Err(format!(
            "属性 0x{property_hash:08X} 的原始值与解析值不一致"
        ));
    }
    out[at + 8] = u8::from(new);
    let patched = PropertyFile::parse(&out).map_err(|e| format!("补丁后解析失败 {e}"))?;
    match patched.get(property_hash).map(|p| &p.kind) {
        Some(Kind::Scalar(Value::Bool(value))) if *value == new => Ok(out),
        actual => Err(format!(
            "属性 0x{property_hash:08X} 补丁校验失败：{actual:?}"
        )),
    }
}

/// 该属性里所有 Key 的 instance（标量或数组皆可），按出现顺序去重。
pub fn key_instances(file: &PropertyFile, hash: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let Some(property) = file.get(hash) else {
        return out;
    };
    let mut push = |k: &crate::Key| {
        if !out.contains(&k.instance) {
            out.push(k.instance);
        }
    };
    match &property.kind {
        Kind::Scalar(Value::Key(k)) => push(k),
        Kind::Array(values) => {
            for value in values {
                if let Value::Key(k) = value {
                    push(k);
                }
            }
        }
        _ => {}
    }
    out
}

/// 该属性里所有 Text 的串 instance（标量或数组皆可），按出现顺序去重。
pub fn text_instances(file: &PropertyFile, hash: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let Some(property) = file.get(hash) else {
        return out;
    };
    let mut push = |t: &crate::Text| {
        if !out.contains(&t.instance_id) {
            out.push(t.instance_id);
        }
    };
    match &property.kind {
        Kind::Scalar(Value::Text(t)) => push(t),
        Kind::Array(values) => {
            for value in values {
                if let Value::Text(t) = value {
                    push(t);
                }
            }
        }
        _ => {}
    }
    out
}

/// 诊断用：u32 列表的十六进制展示。
pub fn hex_list(values: &[u32]) -> String {
    values
        .iter()
        .map(|v| format!("0x{v:08X}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// build_debug_tools_overlay 对缺工具的包必须整体报错（产品命令确定性）。
    #[test]
    fn missing_tool_errors() {
        let dir = std::env::temp_dir().join(format!("openscp-dbg-tools-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.package");
        dbpf::write_uncompressed_overlay_to_path(&path, &[]).unwrap();
        let package = dbpf::Package::open(&path).unwrap();
        let err = build_debug_tools_overlay(&package, DEFAULT_CATEGORY, DEFAULT_UI_CATEGORY)
            .expect_err("应报错");
        assert!(err.contains("未找到工具"), "实际：{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn patch_scalar_bool_preserves_flags_and_only_value_byte() {
        let hash: u32 = 0x0E27_6F5D;
        let mut data = Vec::new();
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&hash.to_be_bytes());
        data.extend_from_slice(&1u16.to_be_bytes());
        data.extend_from_slice(&0x8000u16.to_be_bytes());
        data.push(0);
        let patched = patch_property_bool(&data, hash, false, true).unwrap();
        assert_eq!(patched.len(), data.len());
        assert_eq!(&patched[..12], &data[..12]);
        assert_eq!(patched[12], 1);
    }
}
