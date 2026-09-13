//! 探针：按 InstanceType（group 低 16 位）统计 property 资源分布，并对
//! Agent(0xC600)/Descriptor(0x2043) 等样品 dump 前若干属性，寻找 prop/decal
//! 原型字典类资源。仅开发用。
//!
//! 用法：cargo run -p sc-properties --release --example instancetype_probe -- <package> [dump_n]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

fn main() {
    let path = std::env::args().nth(1).expect("usage: instancetype_probe <package> [dump_n]");
    let dump_n: usize = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(0);
    let package = Package::open(&path).expect("open package");

    let mut counts: BTreeMap<u16, usize> = BTreeMap::new();
    let mut samples: BTreeMap<u16, Vec<u32>> = BTreeMap::new();
    let mut dumped: BTreeMap<u16, usize> = BTreeMap::new();

    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 {
            continue;
        }
        let it = (entry.id.group & 0xFFFF) as u16;
        *counts.entry(it).or_default() += 1;
        let s = samples.entry(it).or_default();
        if s.len() < 6 && !s.contains(&entry.id.instance) {
            s.push(entry.id.instance);
        }
        if dump_n > 0 {
            let d = dumped.entry(it).or_default();
            if *d < dump_n && let Ok(data) = package.read(entry) && let Ok(file) = PropertyFile::parse(&data) {
                *d += 1;
                println!("\n=== InstanceType 0x{it:04X} instance 0x{:08X} group 0x{:08X} ({} props)",
                    entry.id.instance, entry.id.group, file.values.len());
                let mut v: Vec<_> = file.values.iter().collect();
                v.sort_by_key(|p| p.hash);
                for p in v.iter().take(40) {
                    let summary = match &p.kind {
                        Kind::Scalar(sv) => format!("{sv}"),
                        Kind::Array(vals) => {
                            let head: Vec<String> = vals.iter().take(3).map(|x| x.to_string()).collect();
                            format!("array[{}] {}", vals.len(), head.join(", "))
                        }
                        Kind::Empty => "empty".into(),
                    };
                    println!("    0x{:08X} {}: {summary}", p.hash, p.prop_type.name());
                }
            }
        }
    }

    println!("\n--- InstanceType 分布（group & 0xFFFF）---");
    for (it, n) in &counts {
        let s = samples.get(it).cloned().unwrap_or_default();
        let hex: Vec<String> = s.iter().map(|v| format!("0x{v:08X}")).collect();
        println!("  0x{it:04X}: {n:>6} 个  samples=[{}]", hex.join(", "));
    }
    let _ = Value::Bool(true);
}
