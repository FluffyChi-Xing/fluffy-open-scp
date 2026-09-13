//! 实验工具：把三个 debug 工具（Edit Unit Prop Tool / Edit Snap Points Tool /
//! Unit Connecter Tool）的**分类键**重指到可见分类，生成一个未压缩 overlay 包，
//! 放进 `SimCityUserData/Packages/` 由游戏覆盖加载 —— 用于验证「debug 工具是
//! 被数据门控关掉、可重新打开」这一假设。
//!
//! 只改两个字段：
//!   `0x0975695E`（菜单分类 Key，debug = 0x3D752D6A）
//!   `0x0DB9FC63`（UI 子分类 Key，debug = 0xA2A33338 "pipetinetools"）
//! 其余字段（标题/描述/图标/位置/Parent）原样保留。
//!
//! 用法：
//!   cargo run -p sc-exporter --release --example enable_debug_tools -- \
//!       <SimCity_Game.package> <out.package> [--cat=0xC710B6E9] [--ui=0x9D2EF585] [--dry]

use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const TOOL_GROUP: u32 = 0x0987_8A01;
const HASH_TOOL_CATEGORY: u32 = 0x0975_695E;
const HASH_UI_TOOL_CATEGORY: u32 = 0x0DB9_FC63;
const HASH_PARENT: u32 = 0x00B2_CCCB;
const HASH_TITLE: u32 = 0x0A09_F5FA;
const DEBUG_TOOLS: [u32; 3] = [0x53B7_7423, 0xA2AA_4268, 0xB65E_1F2E];
/// **阳性对照**：教育栏里可见的「宿舍」（`ui=0x9D2EF585`，与三个 debug 工具同栏）。
/// 把它的标题串 id 改成同栏「商務學院」的串 id —— 若游戏里真出现两个「商務學院」，
/// 就证明覆盖包确实被加载，前几轮「没出现」不是加载失败导致的假阴性。
const CONTROL_TOOL: u32 = 0x75A9_0B66;
const CONTROL_TITLE_OLD: u32 = 0x7C9A_0071; // 「宿舍」
const CONTROL_TITLE_NEW: u32 = 0x64EC_016B; // 「商務學院」
/// 可见建筑工具（310 条）共同的父对象：一个有 33 个属性的 0xEB00「工具本体」，
/// 自带真实 `ecoGameToolAction`。debug 条目的父对象 `0xBE5744E0` 是 **0 属性空壳**。
const LIVE_TOOL_PARENT: u32 = 0xAF04_2E9A;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let base_path = positional.first().expect("usage: enable_debug_tools <base.package> <out.package> [--cat=] [--ui=] [--dry]");
    let out_path = positional.get(1);
    let dry = args.iter().any(|a| a == "--dry");
    let cat = parse_hex(&args, "--cat=").unwrap_or(0xC710_B6E9);
    let ui = parse_hex(&args, "--ui=").unwrap_or(0x9D2E_F585);
    if !dry && out_path.is_none() {
        panic!("需要输出路径，或加 --dry");
    }

    let package = Package::open(base_path).expect("open base package");
    let mut entries = Vec::new();
    for target in DEBUG_TOOLS {
        let Some(entry) = package.entries().iter().find(|e| {
            e.id.type_id == PROPERTY_TYPE && e.id.group == TOOL_GROUP && e.id.instance == target
        }) else {
            println!("!! 未找到 0x{target:08X}（group 0x{TOOL_GROUP:08X}）");
            continue;
        };
        let data = package.read(entry).expect("read property");
        let file = PropertyFile::parse(&data).expect("parse property");
        let old_cat = key_instances(&file, HASH_TOOL_CATEGORY);
        let old_ui = key_instances(&file, HASH_UI_TOOL_CATEGORY);
        let old_parent = key_instances(&file, HASH_PARENT);
        // 该字段可能有多个 Key（如 0xB65E1F2E 的 UI 分类是两个），全部改写
        let patches: Vec<(u32, u32, u32)> = old_cat
            .iter()
            .map(|old| (HASH_TOOL_CATEGORY, *old, cat))
            .chain(old_ui.iter().map(|old| (HASH_UI_TOOL_CATEGORY, *old, ui)))
            .chain(
                old_parent
                    .iter()
                    .map(|old| (HASH_PARENT, *old, LIVE_TOOL_PARENT)),
            )
            .collect();

        match patch_values(&data, &patches, key_instances) {
            Ok(patched) => {
                println!(
                    "0x{:08X}  Parent {:?} → 0x{:08X}；分类 {:?} → 0x{:08X}；UI 分类 {:?} → 0x{:08X}（{} 字节，{} 处补丁）",
                    target,
                    hex_list(&old_parent),
                    LIVE_TOOL_PARENT,
                    hex_list(&old_cat),
                    cat,
                    hex_list(&old_ui),
                    ui,
                    patched.len(),
                    patches.len()
                );
                entries.push(OverlayEntry::new(
                    ResourceId {
                        type_id: entry.id.type_id,
                        group: entry.id.group,
                        instance: entry.id.instance,
                    },
                    patched,
                ));
            }
            Err(error) => {
                println!("0x{target:08X}  !! 补丁失败，跳过：{error}");
            }
        }
    }
    // ---- 阳性对照：把「宿舍」标题改指到「商務學院」的串 id ----
    if let Some(entry) = package.entries().iter().find(|e| {
        e.id.type_id == PROPERTY_TYPE && e.id.group == TOOL_GROUP && e.id.instance == CONTROL_TOOL
    }) {
        let data = package.read(entry).expect("read control property");
        let patches = [(HASH_TITLE, CONTROL_TITLE_OLD, CONTROL_TITLE_NEW)];
        match patch_values(&data, &patches, text_instances) {
            Ok(patched) => {
                println!(
                    "对照 0x{CONTROL_TOOL:08X} 标题串 0x{CONTROL_TITLE_OLD:08X}（宿舍）→ 0x{CONTROL_TITLE_NEW:08X}（商務學院）"
                );
                entries.push(OverlayEntry::new(entry.id, patched));
            }
            Err(error) => println!("对照 0x{CONTROL_TOOL:08X} !! 补丁失败：{error}"),
        }
    } else {
        println!("对照 0x{CONTROL_TOOL:08X} 未找到");
    }

    if entries.is_empty() {
        panic!("没有任何目标被修改");
    }

    if dry {
        println!("\n--dry：未写文件。目标分类 0x{cat:08X} / UI 分类 0x{ui:08X}");
        return;
    }
    let out_path = out_path.unwrap();
    if let Some(parent) = std::path::Path::new(out_path).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    dbpf::write_uncompressed_overlay_to_path(out_path, &entries).expect("write overlay");
    let size = std::fs::metadata(out_path).map(|m| m.len()).unwrap_or(0);
    println!("\n已写出 {out_path}（{size} 字节，{} 条资源）", entries.len());

    // 自检：重新打开并回读三个条目
    let check = Package::open(out_path).expect("reopen overlay");
    println!("--- 回读校验（{} 条）---", check.entries().len());
    for entry in check.entries() {
        let Ok(data) = check.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        println!(
            "  0x{:08X} g=0x{:08X} 分类=0x{:08X} UI=0x{:08X}",
            entry.id.instance,
            entry.id.group,
            key_instance(&file, HASH_TOOL_CATEGORY).unwrap_or(0),
            key_instance(&file, HASH_UI_TOOL_CATEGORY).unwrap_or(0)
        );
    }
}

/// 逐值字节补丁：把 `(hash, 旧值, 新值)` 里的引用值就地改写（`extract` 决定取
/// Key instance 还是 Text 串 id）。
///
/// **不重编码整份属性**——`encode_canonical` 会对保留的数组元数据做严格校验
///（本文件含 Text 数组，实测报 `InvalidArrayItemSize`），且整包重编码会引入
/// 无关字节差异。就地补丁只动 4 字节，其余与零售包逐字节相同。
///
/// 安全约束：旧值必须在该负载里**恰好出现一次**；补丁后重新解析，要求
/// 「目标 hash 的新值正确」且「其余属性与原文完全一致」，否则整体放弃。
fn patch_values<F>(
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
                (found.len() >= 1).then_some(found)
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

fn parse_hex(args: &[String], prefix: &str) -> Option<u32> {
    args.iter()
        .find_map(|a| a.strip_prefix(prefix))
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
}

/// 该属性里所有 Key 的 instance（标量或数组皆可），按出现顺序去重。
fn key_instances(file: &PropertyFile, hash: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let Some(property) = file.get(hash) else { return out };
    let mut push = |k: &sc_properties::Key| {
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
fn text_instances(file: &PropertyFile, hash: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let Some(property) = file.get(hash) else { return out };
    let mut push = |t: &sc_properties::Text| {
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

fn hex_list(values: &[u32]) -> String {
    values
        .iter()
        .map(|v| format!("0x{v:08X}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// 诊断/自检用：首个 Key 的 instance。
fn key_instance(file: &PropertyFile, hash: u32) -> Option<u32> {
    key_instances(file, hash).first().copied()
}
