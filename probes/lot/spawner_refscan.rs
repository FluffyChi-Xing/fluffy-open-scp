//! 探针：在整包 B1B104 属性里反查一组「名字 id」（Key.instance / UInt32 / Text.instance），
//! 报告命中的文件 TGI、列 hash、值的索引与所在列名（若已知）。仅开发用。
//!
//! 用法：cargo run -p probes --example spawner_refscan -- <package> <id1,id2,...>
use dbpf::Package;
use sc_properties::{PropertyFile, Value};
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: spawner_refscan <package> <ids-csv>");
    let ids: Vec<u32> = args
        .next()
        .expect("ids")
        .split(',')
        .map(|s| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).expect("hex id"))
        .collect();
    let targets: std::collections::BTreeSet<u32> = ids.iter().copied().collect();
    let package = Package::open(&path).expect("open package");

    // 列 hash → 命中统计
    let mut per_col: BTreeMap<u32, usize> = BTreeMap::new();
    let mut per_file: BTreeMap<String, usize> = BTreeMap::new();
    let mut samples: Vec<String> = Vec::new();

    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        for prop in &file.values {
            let mut hits: Vec<(usize, u32)> = Vec::new();
            match &prop.kind {
                sc_properties::Kind::Scalar(v) => {
                    if let Some(h) = scalar_id(v) {
                        if targets.contains(&h) {
                            hits.push((0, h));
                        }
                    }
                }
                sc_properties::Kind::Array(items) => {
                    for (i, v) in items.iter().enumerate() {
                        if let Some(h) = scalar_id(v) {
                            if targets.contains(&h) {
                                hits.push((i, h));
                            }
                        }
                    }
                }
                _ => {}
            }
            if hits.is_empty() {
                continue;
            }
            *per_col.entry(prop.hash).or_default() += hits.len();
            let key = format!(
                "t=0x{:08X} g=0x{:08X} i=0x{:08X}",
                entry.id.type_id, entry.id.group, entry.id.instance
            );
            *per_file.entry(key.clone()).or_default() += hits.len();
            for (i, h) in hits.iter().take(2) {
                if samples.len() < 200 {
                    samples.push(format!(
                        "{key} col=0x{:08X} idx={i} id=0x{h:08X}",
                        prop.hash
                    ));
                }
            }
        }
    }

    println!("--- 命中列（hash × count）");
    for (c, n) in &per_col {
        println!("col 0x{c:08X}  x{n}");
    }
    println!("--- 命中文件 {} 个", per_file.len());
    for (f, n) in per_file.iter().take(40) {
        println!("{f}  x{n}");
    }
    println!("--- 样本");
    for s in &samples {
        println!("{s}");
    }
}

fn scalar_id(v: &Value) -> Option<u32> {
    match v {
        Value::Key(k) => Some(k.instance),
        Value::UInt32(u) => Some(*u),
        Value::Int32(i) => Some(*i as u32),
        _ => None,
    }
}
