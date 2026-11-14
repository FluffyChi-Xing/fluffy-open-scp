//! 探针：扫描包内全部 property，找出含有指定属性 hash 的条目并打印
//! 该 hash 下的值（float/int）。用于定位统计行参数的静态来源。仅开发用。
//!
//! 用法：cargo run -p probes --release --example prop_hash_scan --
//!   <package> <hash_hex> [hash_hex...]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("package");
    let hashes: Vec<u32> = args
        .map(|h| u32::from_str_radix(h.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let p = Package::open(&path).unwrap();
    let mut hits = 0;
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 {
            continue;
        }
        let Ok(data) = p.read(e) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else {
            continue;
        };
        let mut found: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for prop in &file.values {
            if !hashes.contains(&prop.hash) {
                continue;
            }
            let vs: Vec<String> = match &prop.kind {
                Kind::Scalar(v) => vec![format!("{v}")],
                Kind::Array(vs) => vs.iter().map(format_value).collect(),
                Kind::Empty => vec![],
            };
            found.insert(prop.hash, vs);
        }
        if !found.is_empty() {
            hits += 1;
            println!(
                "{:08X}:{:08X}:{:08X}",
                e.id.type_id, e.id.group, e.id.instance
            );
            for (h, vs) in found {
                println!("    {h:08X} = {}", vs.join(", "));
            }
        }
    }
    println!("命中 {hits} 条目");
}

fn format_value(v: &Value) -> String {
    match v {
        Value::Float(f) => format!("{f}"),
        Value::Int32(i) => format!("{i}"),
        Value::UInt32(u) => format!("{u}"),
        Value::Text(t) => format!("text({:08X}:{:08X})", t.table_id, t.instance_id),
        Value::Key(k) => format!("key({:08X})", k.instance),
        other => format!("{other}"),
    }
}
