//! 探针：导出 Menu/Menu2 (0x00B1B104) 全部条目的摘要，用来还原底部工具条的菜单层级。
//!
//! 工具条目的 `0x00B2CCCB`（Parent）指向**一级菜单**，`0x0DB9FC63`（UICategory）指向二级；
//! 这里把所有同类条目的标题文本 id / 图标键 / 位置 / 父级都导出来，层级在导出后 join。
//!
//! 用法：cargo run -p sc-properties --release --example ui_menu_survey -- <out.json> <package...>

use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

const HASH_TITLE: u32 = 0x0A09_F5FA;
const HASH_DESC: u32 = 0x0A09_F5FB;
const HASH_UI_CATEGORY: u32 = 0x0DB9_FC63;
const HASH_UI_POSITION: u32 = 0x0DC1_E3E0;
const HASH_PARENT: u32 = 0x00B2_CCCB;
const HASH_ICON_NORMAL: u32 = 0x0975_6950;
const HASH_HARD_GATE: u32 = 0x0975_695F;
const TYPE_PROPERTY: u32 = 0x00B1_B104;

/// 工具面板 / 分类对象字段（名取自 app_main.js 的 `kPropTool*` 常量）。
const EXTRA_FIELDS: &[(&str, u32)] = &[
    ("toolCategoryName", 0x0C2B_F463),
    ("toolCategoryIconKeys", 0x0AB8_C50C),
    ("toolCategoryOrder", 0xCDCE_F1AF),
    ("toolCategoryGroupID", 0x0C25_85B1),
    ("toolCategoryPaletteLayout", 0x0C22_F4DE),
    ("toolCategoryToolList", 0x0AFB_A88D),
    ("toolPaletteCategoryIDs", 0x09A4_A024),
    ("toolPaletteCategoryOffsets", 0x0C85_D445),
    ("toolPaletteCategoryIsDebug", 0x0D11_5956),
    ("toolMarqueeImage", 0x0DDE_EE56),
    // 菜单条目（8A01）的槽位图标：等轴小模型渲染图（PNG 组 40E02400）。
    // 0x0975_6950..55 的六态图标键在包里按 TGI 找不到资源（见 2026-09 复盘），
    // 真正被 UI 槽位用的是 kPropToolIconKey。
    ("toolIconKey", 0x0977_AA8F),
    // 解锁条件（弹窗里的「達到上限/不批准」说明）
    ("toolUnlockString", 0x0DE8_4DDC),
    ("toolUnlockTargetAmount", 0x0DE8_4DD3),
    // rollover 统计行：Title = 标签模板（locale 文本，含 ~amount:number~ 运行时占位），
    // Parameter = SCUnit 统计 id（数值由模拟引擎运行时填充，包内无静态值）
    ("unitEffectTitle", 0x0EB1_FC05),
    ("unitEffectParameter", 0x0EB1_FC22),
];

const TYPE_PNG: u32 = 0x2F7D_0004;
const TYPE_JPG: u32 = 0x3F86_62EA;

fn key_instances(file: &PropertyFile, hash: u32) -> Vec<(u32, u32, u32)> {
    let Some(entry) = file.get(hash) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    match &entry.kind {
        Kind::Scalar(Value::Key(k)) => out.push((k.type_id, k.group, k.instance)),
        Kind::Array(values) => {
            for v in values {
                if let Value::Key(k) = v {
                    out.push((k.type_id, k.group, k.instance));
                }
            }
        }
        _ => {}
    }
    out
}

fn text_id(file: &PropertyFile, hash: u32) -> Option<u32> {
    match &file.get(hash)?.kind {
        Kind::Scalar(Value::Text(t)) => Some(t.instance_id),
        Kind::Array(values) => values.iter().find_map(|v| match v {
            Value::Text(t) => Some(t.instance_id),
            _ => None,
        }),
        _ => None,
    }
}

fn i32_at(file: &PropertyFile, hash: u32) -> Option<i32> {
    match &file.get(hash)?.kind {
        Kind::Scalar(Value::Int32(v)) => Some(*v),
        Kind::Array(values) => values.first().and_then(|v| match v {
            Value::Int32(n) => Some(*n),
            _ => None,
        }),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out_json = PathBuf::from(args.first().expect("usage: ui_menu_survey <out.json> <package...>"));
    let mut packages: Vec<String> = Vec::new();
    // `--dump <typehex>[:<n>]` 只打印该 InstanceType 前 n 条的完整属性，用来摸字段 schema。
    let mut dump_type: Option<(u32, usize)> = None;
    // `--has <hashhex>[:<n>]`：只打印含该属性哈希的条目（用来找一级菜单表等）。
    let mut has_filter: Option<(u32, usize)> = None;
    // `--extract <dir>`：把条目引用的预览图/图标按 TGI 抠出来，文件名 <TYPE>-<GROUP>-<INSTANCE>.<ext>
    let mut extract_dir: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--extract" {
            extract_dir = args.get(i + 1).map(PathBuf::from);
            i += 2;
        } else if args[i] == "--dump" {
            let spec = args.get(i + 1).cloned().unwrap_or_default();
            let (t, n) = spec.split_once(':').unwrap_or((spec.as_str(), "1"));
            dump_type = Some((
                u32::from_str_radix(t, 16).unwrap_or(0),
                n.parse().unwrap_or(1),
            ));
            i += 2;
        } else if args[i] == "--has" {
            let spec = args.get(i + 1).cloned().unwrap_or_default();
            let (t, n) = spec.split_once(':').unwrap_or((spec.as_str(), "1"));
            has_filter = Some((
                u32::from_str_radix(t, 16).unwrap_or(0),
                n.parse().unwrap_or(1),
            ));
            i += 2;
        } else {
            packages.push(args[i].clone());
            i += 1;
        }
    }

    let mut rows: Vec<BTreeMap<String, String>> = Vec::new();
    let mut groups: BTreeMap<u32, usize> = BTreeMap::new();
    let mut dumped = 0usize;

    for path in &packages {
        let Ok(package) = dbpf::Package::open(path) else {
            eprintln!("skip {path}");
            continue;
        };
        for entry in package.entries() {
            if entry.id.type_id != TYPE_PROPERTY {
                continue;
            }
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else { continue };
            let instance_type = (entry.id.group & 0xFFFF) as u32;
            *groups.entry(instance_type).or_default() += 1;

            if let Some((t, n)) = dump_type {
                if t == instance_type && dumped < n {
                    dumped += 1;
                    println!(
                        "===== instance {:08X}  type {:04X}  group {:08X} =====",
                        entry.id.instance, instance_type, entry.id.group
                    );
                    print!("{file}");
                }
            }
            if let Some((h, n)) = has_filter {
                if dumped < n && file.get(h).is_some() {
                    dumped += 1;
                    println!(
                        "===== HAS {h:08X}: instance {:08X}  type {:04X}  group {:08X} =====",
                        entry.id.instance, instance_type, entry.id.group
                    );
                    print!("{file}");
                }
            }

            let mut row: BTreeMap<String, String> = BTreeMap::new();
            row.insert("instance".into(), format!("{:08X}", entry.id.instance));
            row.insert("instanceType".into(), format!("{instance_type:04X}"));
            row.insert("group".into(), format!("{:08X}", entry.id.group));
            if let Some(t) = text_id(&file, HASH_TITLE) {
                row.insert("titleTextId".into(), format!("{t:08X}"));
            }
            if let Some(t) = text_id(&file, HASH_DESC) {
                row.insert("descTextId".into(), format!("{t:08X}"));
            }
            let cats: Vec<String> = key_instances(&file, HASH_UI_CATEGORY)
                .iter()
                .map(|(_, _, i)| format!("{i:08X}"))
                .collect();
            if !cats.is_empty() {
                row.insert("uiCategories".into(), cats.join(","));
            }
            if let Some((_, _, pi)) = key_instances(&file, HASH_PARENT).first() {
                row.insert("parent".into(), format!("{pi:08X}"));
            }
            if let Some(pos) = i32_at(&file, HASH_UI_POSITION) {
                row.insert("uiPosition".into(), pos.to_string());
            }
            if let Some((t, g, i)) = key_instances(&file, HASH_ICON_NORMAL).first() {
                row.insert("iconNormal".into(), format!("{t:08X}-{g:08X}-{i:08X}"));
            }
            if file.get(HASH_HARD_GATE).is_some() {
                row.insert("hardGate".into(), "1".into());
            }
            // 工具面板 / 分类对象用到的字段（见 app_main.js 的 kPropTool* 常量）
            for (name, hash) in EXTRA_FIELDS {
                if let Some(entry) = file.get(*hash) {
                    let v = match &entry.kind {
                        Kind::Scalar(Value::Key(k)) => format!("{:08X}-{:08X}-{:08X}", k.type_id, k.group, k.instance),
                        Kind::Scalar(Value::Int32(n)) => n.to_string(),
                        Kind::Scalar(Value::Float(f)) => f.to_string(),
                        Kind::Scalar(Value::Text(t)) => format!("text:{:08X}", t.instance_id),
                        Kind::Scalar(Value::Bool(b)) => b.to_string(),
                        Kind::Array(vals) => {
                            let items: Vec<String> = vals
                                .iter()
                                .map(|v| match v {
                                    Value::Key(k) => format!("{:08X}", k.instance),
                                    Value::Int32(n) => n.to_string(),
                                    Value::UInt32(n) => n.to_string(),
                                    Value::Float(f) => f.to_string(),
                                    Value::Text(t) => format!("text:{:08X}", t.instance_id),
                                    Value::Bool(b) => b.to_string(),
                                    other => format!("{other:?}"),
                                })
                                .collect();
                            items.join(",")
                        }
                        other => format!("{other:?}"),
                    };
                    row.insert((*name).into(), v);
                }
            }
            rows.push(row);
        }
    }

    eprintln!("InstanceType 分布:");
    for (k, n) in &groups {
        eprintln!("   {k:04X}: {n}");
    }

    let body: Vec<String> = rows
        .iter()
        .map(|row| {
            let fields: Vec<String> = row.iter().map(|(k, v)| format!("\"{k}\": \"{v}\"")).collect();
            format!("    {{{}}}", fields.join(", "))
        })
        .collect();
    let json = format!(
        "{{\n  \"count\": {},\n  \"entries\": [\n{}\n  ]\n}}\n",
        rows.len(),
        body.join(",\n")
    );
    std::fs::write(&out_json, json).expect("write out");
    eprintln!("wrote {} entries -> {}", rows.len(), out_json.display());

    if let Some(dir) = extract_dir {
        std::fs::create_dir_all(&dir).expect("create extract dir");
        // 收集条目里引用的图片 TGI（iconNormal 六态键多数不落地，toolIconKey 才是槽位图）
        let mut wanted: std::collections::HashSet<(u32, u32, u32)> = std::collections::HashSet::new();
        for row in &rows {
            for key in ["toolMarqueeImage", "iconNormal", "toolIconKey"] {
                let Some(v) = row.get(key) else { continue };
                let parts: Vec<&str> = v.split('-').collect();
                if parts.len() != 3 {
                    continue;
                }
                if let (Ok(t), Ok(g), Ok(i)) = (
                    u32::from_str_radix(parts[0], 16),
                    u32::from_str_radix(parts[1], 16),
                    u32::from_str_radix(parts[2], 16),
                ) {
                    wanted.insert((t, g, i));
                }
            }
        }
        let mut n = 0usize;
        for path in &packages {
            let Ok(package) = dbpf::Package::open(path) else { continue };
            for entry in package.entries() {
                if entry.id.type_id != TYPE_PNG && entry.id.type_id != TYPE_JPG {
                    continue;
                }
                let tgi = (entry.id.type_id, entry.id.group, entry.id.instance);
                if !wanted.contains(&tgi) {
                    continue;
                }
                let ext = if entry.id.type_id == TYPE_PNG { "png" } else { "jpg" };
                let out = dir.join(format!("{:08X}-{:08X}-{:08X}.{}", tgi.0, tgi.1, tgi.2, ext));
                if out.exists() {
                    continue;
                }
                if let Ok(data) = package.read(entry) {
                    if std::fs::write(&out, &data).is_ok() {
                        n += 1;
                    }
                }
            }
        }
        eprintln!("extracted {n} images -> {}", dir.display());
    }
}
