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
            .chain(old_parents.iter().map(|old| (HASH_PARENT, *old, LIVE_TOOL_PARENT)))
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

/// 该属性里所有 Key 的 instance（标量或数组皆可），按出现顺序去重。
pub fn key_instances(file: &PropertyFile, hash: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let Some(property) = file.get(hash) else { return out };
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
    let Some(property) = file.get(hash) else { return out };
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
}
