//! 探针：40E1C000（资产单元配置）条目的 名称文本id + Cost + 维护，
//! 用于把 cost 条目反查回菜单条目（共享同一 6C969DEE 名称字符串）。仅开发用。
//!
//! 用法：cargo run -p sc-properties --release --example unit_config_name_scan -- <package> [名称文本id]
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

const TYPE_PROPERTY: u32 = 0x00B1_B104;
const GROUP_UNIT_CONFIG: u32 = 0x40E1_C000;
const HASH_COST: u32 = 0x0C8E_E92F;
const HASH_MAINT: u32 = 0x09AE_19D7;
const HASH_CONSTRUCTION: u32 = 0x0A8A_41DA;

fn i32_of(file: &PropertyFile, hash: u32) -> Option<i32> {
    match &file.get(hash)?.kind {
        Kind::Scalar(v) => i32_val(v),
        Kind::Array(values) => values.iter().find_map(i32_val),
        _ => None,
    }
}

fn i32_val(v: &Value) -> Option<i32> {
    match v {
        Value::Int32(n) => Some(*n),
        Value::UInt32(n) => Some(*n as i32),
        Value::Float(f) => Some(*f as i32),
        _ => None,
    }
}

/// 条目里全部 6C969DEE 文本引用（名称候选）。
fn name_ids(file: &PropertyFile) -> Vec<u32> {
    let mut out = Vec::new();
    for entry in &file.values {
        let refs: Vec<&Value> = match &entry.kind {
            Kind::Scalar(v) => vec![v],
            Kind::Array(values) => values.iter().collect(),
            Kind::Empty => Vec::new(),
        };
        for v in refs {
            if let Value::Text(t) = v {
                if t.table_id == 0x6C96_9DEE {
                    out.push(t.instance_id);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: unit_config_name_scan <package> [name-text-id]");
    let want: Option<u32> = args
        .next()
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
    let package = dbpf::Package::open(&path).expect("open package");
    let mut rows: BTreeMap<u32, (Vec<u32>, Option<i32>, Option<i32>)> = BTreeMap::new();
    for entry in package.entries() {
        if entry.id.type_id != TYPE_PROPERTY || entry.id.group != GROUP_UNIT_CONFIG {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        let ids = name_ids(&file);
        if let Some(want) = want {
            if !ids.contains(&want) {
                continue;
            }
        }
        rows.insert(
            entry.id.instance,
            (ids, i32_of(&file, HASH_COST).or(i32_of(&file, HASH_CONSTRUCTION)), i32_of(&file, HASH_MAINT)),
        );
    }
    for (instance, (ids, cost, maint)) in &rows {
        let ids = ids.iter().map(|i| format!("{i:08X}")).collect::<Vec<_>>().join(",");
        println!("{:08X}  cost={:?} maint={:?}  names=[{}]", instance, cost, maint, ids);
    }
    println!("-- 共 {} 条", rows.len());
}
