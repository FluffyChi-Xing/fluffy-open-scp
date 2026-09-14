//! 探针：导出全部 Menu/Menu2 工具定义（0x00B1B104, InstanceType 0x8A01/0xC900）
//! 为 JSON，供 UI 工作台按游戏真实数据装配工具条。
//! 同时把六态图标（0x09756950–55 指向的 PNG/JPG）提取到 <out-dir>/。
//!
//! 用法：cargo run -p sc-properties --release --example ui_tool_dump -- <out-json> <icon-dir> <package...>

use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

const HASH_TITLE: u32 = 0x0A09_F5FA;
const HASH_DESC: u32 = 0x0A09_F5FB;
const HASH_UI_CATEGORY: u32 = 0x0DB9_FC63;
const HASH_UI_POSITION: u32 = 0x0DC1_E3E0;
const HASH_PARENT: u32 = 0x00B2_CCCB;
const HASH_ICON_NORMAL: u32 = 0x0975_6950;
const HASH_ICON_LOCKED: u32 = 0x0975_6955;
const HASH_HARD_GATE: u32 = 0x0975_695F;

const TYPE_PROPERTY: u32 = 0x00B1_B104;
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
    let out_json = PathBuf::from(args.first().expect("usage: ui_tool_dump <out.json> <icon-dir> <package...>"));
    let icon_dir = PathBuf::from(args.get(1).expect("missing icon-dir"));
    std::fs::create_dir_all(&icon_dir).expect("create icon dir");
    let packages = &args[2..];

    let mut tools: Vec<BTreeMap<String, String>> = Vec::new();
    let mut wanted_icons: std::collections::HashSet<(u32, u32, u32)> =
        std::collections::HashSet::new();

    // 续跑模式：已有 tools.json 时沿用其 iconStates（工具只从主安装枚举，
    // 图标可再喂 Locale/离线包补齐提取）。
    if out_json.exists() {
        if let Ok(existing) = std::fs::read_to_string(&out_json) {
            for cap in existing.split("iconStates\": \"").skip(1) {
                let field = cap.split('"').next().unwrap_or("");
                for kv in field.split(',') {
                    let Some((_, tgi)) = kv.split_once(':') else { continue };
                    let mut parts = tgi.split('-');
                    let (Some(t), Some(g), Some(i)) = (parts.next(), parts.next(), parts.next())
                    else {
                        continue;
                    };
                    if let (Ok(t), Ok(g), Ok(i)) = (
                        u32::from_str_radix(t, 16),
                        u32::from_str_radix(g, 16),
                        u32::from_str_radix(i, 16),
                    ) {
                        wanted_icons.insert((t, g, i));
                    }
                }
            }
        }
    }

    for path in packages {
        let Ok(package) = dbpf::Package::open(path) else {
            eprintln!("skip {path}");
            continue;
        };
        for entry in package.entries() {
            if entry.id.type_id != TYPE_PROPERTY {
                continue;
            }
            let instance_type = (entry.id.group & 0xFFFF) as u16;
            if instance_type != 0x8A01 && instance_type != 0xC900 {
                continue;
            }
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else { continue };
            let mut row: BTreeMap<String, String> = BTreeMap::new();
            row.insert("instance".into(), format!("{:08X}", entry.id.instance));
            row.insert("menuType".into(), format!("{instance_type:04X}"));
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
            if let Some(pos) = i32_at(&file, HASH_UI_POSITION) {
                row.insert("uiPosition".into(), pos.to_string());
            }
            if let Some((_, _, pi)) = key_instances(&file, HASH_PARENT).first() {
                row.insert("parent".into(), format!("{pi:08X}"));
            }
            // 六态图标：0x09756950..55（Key 携带完整 TGI，图标 PNG 在 group 0x40E02400）
            let mut states = Vec::new();
            for (offset, state) in (0u32..6).map(|i| (HASH_ICON_NORMAL + i, i)) {
                if let Some((t, g, i)) = key_instances(&file, offset).first() {
                    states.push(format!("{state}:{t:08X}-{g:08X}-{i:08X}"));
                    wanted_icons.insert((*t, *g, *i));
                }
            }
            if !states.is_empty() {
                row.insert("iconStates".into(), states.join(","));
            }
            if file.get(HASH_HARD_GATE).is_some() {
                row.insert("hardGate".into(), "1".into());
            }
            tools.push(row);
        }
    }

    // 提取图标：按完整 TGI 精确匹配
    let mut extracted = 0usize;
    for path in packages {
        let Ok(package) = dbpf::Package::open(path) else { continue };
        for entry in package.entries() {
            if entry.id.type_id != TYPE_PNG && entry.id.type_id != TYPE_JPG {
                continue;
            }
            let tgi = (entry.id.type_id, entry.id.group, entry.id.instance);
            if !wanted_icons.contains(&tgi) {
                continue;
            }
            let ext = if entry.id.type_id == TYPE_PNG { "png" } else { "jpg" };
            let out = icon_dir.join(format!("{:08X}-{ext}", entry.id.instance));
            if out.exists() {
                continue;
            }
            if let Ok(data) = package.read(entry) {
                if std::fs::write(&out, &data).is_ok() {
                    extracted += 1;
                }
            }
        }
    }

    // 稳定排序：menuType, uiPosition, instance
    tools.sort_by(|a, b| {
        a.get("uiPosition")
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(i32::MAX)
            .cmp(&b.get("uiPosition").and_then(|v| v.parse::<i32>().ok()).unwrap_or(i32::MAX))
            .then_with(|| a.get("instance").cmp(&b.get("instance")))
    });

    let rows: Vec<String> = tools
        .iter()
        .map(|row| {
            let fields: Vec<String> = row
                .iter()
                .map(|(k, v)| format!("\"{k}\": \"{v}\""))
                .collect();
            format!("    {{{}}}", fields.join(", "))
        })
        .collect();
    let json = format!(
        "{{\n  \"source\": \"Menu/Menu2 (0x00B1B104 InstanceType 0x8A01/0xC900)\",\n  \"count\": {},\n  \"tools\": [\n{}\n  ]\n}}\n",
        tools.len(),
        rows.join(",\n")
    );
    std::fs::write(&out_json, json).expect("write tools json");
    println!("tools={} icons_extracted={extracted}", tools.len());
}
