//! 只读分析：0x00B1B104:40E1E800 族 property 的结构与引用调查。
//!
//! 子命令：
//!   dump   <package> <instance>            全量打印一个 property 的键值
//!   refs   <package> <instance> [lookup..] 解析其中引用的全部 TGI → 目标资源类型
//!   survey <package>                        0x00B1B104 按 group 的占比统计

use dbpf::{IndexEntry, Package};
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args[0].as_str() {
        "dump" => dump(&args[1], parse_hex(&args[2])),
        "refs" => refs(&args[1], parse_hex(&args[2]), &args[3..]),
        "survey" => survey(&args[1]),
        other => panic!("unknown subcommand {other}"),
    }
}

fn parse_hex(v: &str) -> u32 {
    u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap()
}

fn open(path: &str) -> Package {
    Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"))
}

fn find<'a>(
    packages: &'a [Package],
    type_id: u32,
    instance: u32,
    group: Option<u32>,
) -> Option<(usize, IndexEntry)> {
    for (index, package) in packages.iter().enumerate() {
        for entry in package.entries() {
            if entry.id.type_id == type_id
                && entry.id.instance == instance
                && group.is_none_or(|g| entry.id.group == g)
            {
                return Some((index, entry.clone()));
            }
        }
    }
    None
}

fn dump(package_path: &str, instance: u32) {
    let package = open(package_path);
    let (owner, entry) = find(std::slice::from_ref(&package), 0x00B1_B104, instance, None)
        .unwrap_or_else(|| panic!("not found"));
    println!(
        "property T 0x{:08X} G 0x{:08X} I 0x{:08X}  pkg#{}  stored={}B decompressed={}B",
        entry.id.type_id,
        entry.id.group,
        entry.id.instance,
        owner,
        entry.compressed_size,
        entry.decompressed_size
    );
    let data = package.read(&entry).unwrap();
    let file = PropertyFile::parse(&data).expect("parse");
    println!("entries = {} (claimed {})", file.values.len(), file.claimed_count);
    let mut keys_with_keys = 0usize;
    let mut key_values = 0usize;
    for property in &file.values {
        let hash = property.hash;
        match &property.kind {
            Kind::Scalar(value) => {
                if let Value::Key(key) = value {
                    keys_with_keys += 1;
                    key_values += 1;
                    println!(
                        "  0x{hash:08X}  KEY   T {:08X} G {:08X} I {:08X}",
                        key.type_id, key.group, key.instance
                    );
                } else {
                    println!("  0x{hash:08X}  {} = {}", kind_name(&property.kind), value);
                }
            }
            Kind::Array(values) => {
                let key_count = values
                    .iter()
                    .filter(|v| matches!(v, Value::Key(_)))
                    .count();
                key_values += key_count;
                if key_count > 0 {
                    keys_with_keys += 1;
                }
                println!(
                    "  0x{hash:08X}  ARRAY[{}] of {}{}",
                    values.len(),
                    kind_name(&property.kind),
                    if key_count > 0 {
                        format!("（含 {key_count} 个 TGI）")
                    } else {
                        String::new()
                    }
                );
                for value in values.iter().take(6) {
                    println!("      - {value}");
                }
                if values.len() > 6 {
                    println!("      - …（{} more）", values.len() - 6);
                }
            }
            Kind::Empty => println!("  0x{hash:08X}  EMPTY"),
        }
    }
    println!("summary: {keys_with_keys} 个键含 TGI 引用，共 {key_values} 个 TGI 值");
}

fn refs(package_path: &str, instance: u32, lookups: &[String]) {
    let mut paths = vec![package_path.to_string()];
    paths.extend(lookups.iter().cloned());
    let packages: Vec<Package> = paths.iter().map(|p| open(p)).collect();
    let (owner, entry) = find(&packages, 0x00B1_B104, instance, None).unwrap();
    let data = packages[owner].read(&entry).unwrap();
    let file = PropertyFile::parse(&data).expect("parse");

    let mut refs: BTreeMap<(u32, u32, u32), Vec<u32>> = BTreeMap::new();
    for property in &file.values {
        let keys: Vec<sc_properties::Key> = match &property.kind {
            Kind::Scalar(Value::Key(k)) => vec![*k],
            Kind::Array(values) => values
                .iter()
                .filter_map(|v| match v {
                    Value::Key(k) => Some(*k),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        };
        for key in keys {
            refs.entry((key.type_id, key.group, key.instance))
                .or_default()
                .push(property.hash);
        }
    }
    println!("distinct TGIs = {}", refs.len());
    for ((type_id, group, instance), hashes) in &refs {
        let mut found = String::from("NOT FOUND");
        'outer: for (index, package) in packages.iter().enumerate() {
            for e in package.entries() {
                let type_match = *type_id == 0 || e.id.type_id == *type_id;
                let group_match = *group == 0 || e.id.group == *group;
                if e.id.instance == *instance && type_match && group_match {
                    found = format!(
                        "pkg#{} T {:08X} G {:08X} ({}B)",
                        index, e.id.type_id, e.id.group, e.decompressed_size
                    );
                    break 'outer;
                }
            }
        }
        println!(
            "  T {type_id:08X} G {group:08X} I {instance:08X} ← keys {hashes:?} → {found}"
        );
    }
}

fn survey(package_path: &str) {
    let package = open(package_path);
    let entries = package.entries();
    let total = entries.len();
    let total_bytes: u64 = entries.iter().map(|e| u64::from(e.decompressed_size)).sum();
    let mut lot_groups: BTreeMap<u32, (usize, u64)> = BTreeMap::new();
    let mut lot_count = 0usize;
    let mut lot_bytes = 0u64;
    for entry in entries.iter() {
        if entry.id.type_id == 0x00B1_B104 {
            lot_count += 1;
            lot_bytes += u64::from(entry.decompressed_size);
            let slot = lot_groups.entry(entry.id.group).or_default();
            slot.0 += 1;
            slot.1 += u64::from(entry.decompressed_size);
        }
    }
    println!("package 总资源 = {total}，总解压字节 = {total_bytes}");
    let lot_pct = 100.0 * lot_count as f64 / total as f64;
    let lot_byte_pct = 100.0 * lot_bytes as f64 / total_bytes as f64;
    println!(
        "0x00B1B104 数量 = {lot_count}（占 {lot_pct:.2}%），字节 = {lot_bytes}（占 {lot_byte_pct:.2}%）"
    );
    println!("按 group 分布：");
    for (group, (count, bytes)) in &lot_groups {
        let pct = 100.0 * *bytes as f64 / lot_bytes as f64;
        println!("  G {group:08X}: {count} 个，{bytes} B（占本类 {pct:.2}%）");
    }
}

fn kind_name(kind: &Kind) -> &'static str {
    match kind {
        Kind::Array(_) => "ARRAY",
        Kind::Empty => "EMPTY",
        Kind::Scalar(_) => "SCALAR",
    }
}
