//! 探针：普查 Menu / Menu2（InstanceType 0xC900 / 0x8A01）工具定义的两个分类键，
//! 用于找出「可见工具」的分类取值作为重指参考。仅开发用。
//!
//! 用法：cargo run -p sc-properties --release --example menu_survey -- <package> [--cat=<hex>]

use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

const HASH_TOOL_CATEGORY: u32 = 0x0975_695E;
const HASH_UI_TOOL_CATEGORY: u32 = 0x0DB9_FC63;
const HASH_TITLE: u32 = 0x0A09_F5FA;
const HASH_UI_POSITION: u32 = 0x0DC1_E3E0;
const HASH_PARENT: u32 = 0x00B2_CCCB;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: menu_survey <package> [--cat=0x...]");
    let filter: Option<u32> = args
        .iter()
        .find_map(|a| a.strip_prefix("--cat="))
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
    let package = dbpf::Package::open(path).expect("open package");

    let mut cat_hist: BTreeMap<u32, usize> = BTreeMap::new();
    let mut ui_hist: BTreeMap<u32, usize> = BTreeMap::new();
    let mut pairs: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    let mut parent_hist: BTreeMap<u32, usize> = BTreeMap::new();
    let mut prop_count_hist: BTreeMap<usize, usize> = BTreeMap::new();
    let mut total = 0usize;

    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 {
            continue;
        }
        let instance_type = (entry.id.group & 0xFFFF) as u16;
        if instance_type != 0x8A01 && instance_type != 0xC900 {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        let category = key_instance(&file, HASH_TOOL_CATEGORY);
        let ui = key_instance(&file, HASH_UI_TOOL_CATEGORY);
        let title = text_id(&file, HASH_TITLE);
        let position = i32_at(&file, HASH_UI_POSITION);
        let parent = key_instance(&file, HASH_PARENT);
        total += 1;
        if let Some(p) = parent {
            *parent_hist.entry(p).or_default() += 1;
        }
        *prop_count_hist.entry(file.values.len()).or_default() += 1;
        if let Some(c) = category {
            *cat_hist.entry(c).or_default() += 1;
        }
        if let Some(u) = ui {
            *ui_hist.entry(u).or_default() += 1;
        }
        if let (Some(c), Some(u)) = (category, ui) {
            *pairs.entry((c, u)).or_default() += 1;
        }
        if let Some(want) = filter
            && category == Some(want)
        {
            println!(
                "  I=0x{:08X} cat=0x{:08X} ui=0x{:08X} pos={:?} title=0x{:08X}",
                entry.id.instance,
                category.unwrap_or(0),
                ui.unwrap_or(0),
                position,
                title.unwrap_or(0)
            );
        }
    }

    println!("menu/menu2 条目总数 = {total}");
    println!("--- 0x0975695E（菜单分类）分布 ---");
    for (k, v) in &cat_hist {
        println!("    0x{k:08X} × {v}");
    }
    println!("--- 0x0DB9FC63（UI 分类）分布 ---");
    for (k, v) in &ui_hist {
        println!("    0x{k:08X} × {v}");
    }
    println!("--- Parent(0x00B2CCCB) 分布（前 20）---");
    let mut parents: Vec<(&u32, &usize)> = parent_hist.iter().collect();
    parents.sort_by(|a, b| b.1.cmp(a.1));
    for (k, v) in parents.into_iter().take(20) {
        println!("    parent=0x{k:08X} × {v}");
    }
    println!("--- 属性条数分布 ---");
    let mut counts: Vec<(&usize, &usize)> = prop_count_hist.iter().collect();
    counts.sort_by_key(|(k, _)| **k);
    for (k, v) in counts {
        println!("    {k} 个属性 × {v}");
    }
    println!("--- (菜单分类, UI 分类) 组合 ---");
    for ((c, u), v) in &pairs {
        println!("    cat=0x{c:08X} ui=0x{u:08X} × {v}");
    }
}

fn key_instance(file: &PropertyFile, hash: u32) -> Option<u32> {
    match &file.get(hash)?.kind {
        Kind::Scalar(Value::Key(k)) => Some(k.instance),
        Kind::Array(values) => values.iter().find_map(|v| match v {
            Value::Key(k) => Some(k.instance),
            _ => None,
        }),
        _ => None,
    }
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
        Kind::Array(values) => values.iter().find_map(|v| match v {
            Value::Int32(n) => Some(*n),
            _ => None,
        }),
        _ => None,
    }
}
