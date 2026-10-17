//! 探针：扫描 40E1C000（资产单元配置）组 property 的 Cost（0x0C8EE92F）等经济字段，
//! 用于定位弹窗里造价/维护费的数据来源。仅开发用。
//!
//! 用法：cargo run -p sc-properties --release --example unit_cost_scan -- <package> [目标Cost值]
//!      cargo run -p sc-properties --release --example unit_cost_scan -- <package> --instance <hex>

use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

const TYPE_PROPERTY: u32 = 0x00B1_B104;
const HASH_COST: u32 = 0x0C8E_E92F;
const HASH_MAINT: u32 = 0x09AE_19D7;
const HASH_CONSTRUCTION: u32 = 0x0A8A_41DA;
const HASH_MODEL: u32 = 0x0953_2375;

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

fn key_instance(file: &PropertyFile, hash: u32) -> Option<u32> {
    match &file.get(hash)?.kind {
        Kind::Scalar(Value::Key(key)) => Some(key.instance),
        Kind::Array(values) => values.iter().find_map(|value| match value {
            Value::Key(key) => Some(key.instance),
            _ => None,
        }),
        _ => None,
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: unit_cost_scan <package> [cost]");
    let mut want: Option<i32> = None;
    let mut instance: Option<u32> = None;
    let mut model: Option<u32> = None;
    while let Some(arg) = args.next() {
        if arg == "--instance" {
            instance = args.next().and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
        } else if arg == "--model" {
            model = args.next().and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
        } else {
            want = arg.parse().ok();
        }
    }
    let package = dbpf::Package::open(&path).expect("open package");
    let mut dist: BTreeMap<i32, Vec<u32>> = BTreeMap::new();
    for entry in package.entries() {
        if entry.id.type_id != TYPE_PROPERTY {
            continue;
        }
        if let Some(instance) = instance && entry.id.instance != instance {
            continue;
        }
        let Ok(data) = package.read(&entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        if let Some(model) = model && key_instance(&file, HASH_MODEL) != Some(model) {
            continue;
        }
        let cost = i32_of(&file, HASH_COST);
        if let Some(c) = cost {
            dist.entry(c).or_default().push(entry.id.instance);
            if instance.is_some() || model.is_some() || want == Some(c) {
                println!(
                    "instance {:08X} cost={} maint={:?} constr={:?}",
                    entry.id.instance,
                    c,
                    i32_of(&file, HASH_MAINT),
                    i32_of(&file, HASH_CONSTRUCTION)
                );
            }
            if want == Some(c) {
                println!("=== full dump of {:08X} ===", entry.id.instance);
                print!("{file}");
            }
        }
    }
    if want.is_none() {
        println!("cost 值分布（前 30）:");
        for (cost, insts) in dist.iter().take(30) {
            println!("  {}: {} 个，例 {:08X}", cost, insts.len(), insts[0]);
        }
    }
}
