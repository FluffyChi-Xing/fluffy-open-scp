//! 探针：把 EcoGame transport entry 全量导出为 JSON（每条 entry 的所有列 hash + 值），
//! 供 Python 侧做「哪些 entry 带 sim 外观列 / 活动文案列」的统计。仅开发用。
//!
//! 用法：cargo run -p probes --example simm_transport_dump -- <package> [out.json]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
use std::io::Write;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: simm_transport_dump <package> [out.json]");
    let out = args
        .next()
        .unwrap_or_else(|| "transport_dump.json".into());
    let package = Package::open(&path).expect("open package");

    // 1) master 的 transport group 列表
    let mut groups = std::collections::BTreeSet::new();
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        if let Some(prop) = file.get(0x0A30_A558) {
            if let Kind::Array(items) = &prop.kind {
                for v in items {
                    if let Value::Key(k) = v {
                        groups.insert(k.group);
                    }
                }
            }
        }
    }

    let mut rows: Vec<String> = Vec::new();
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 || !groups.contains(&entry.id.group) {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        let mut cols: Vec<String> = Vec::new();
        for prop in &file.values {
            let desc = match &prop.kind {
                Kind::Scalar(Value::Key(k)) => format!("k:{}:{}", k.instance, k.group),
                Kind::Scalar(Value::UInt32(u)) => format!("u:{u}"),
                Kind::Scalar(Value::Int32(i)) => format!("i:{i}"),
                Kind::Scalar(Value::Float(f)) => format!("f:{f}"),
                Kind::Scalar(Value::Bool(b)) => format!("b:{}", if *b { 1 } else { 0 }),
                Kind::Scalar(Value::String8(s)) | Kind::Scalar(Value::String16(s)) => {
                    format!("s:{s}")
                }
                Kind::Scalar(Value::Text(t)) => format!("x:{}:{}", t.table_id, t.instance_id),
                Kind::Array(items) => {
                    let head: Vec<String> = items
                        .iter()
                        .take(8)
                        .map(|v| match v {
                            Value::Key(k) => format!("{}", k.instance),
                            Value::UInt32(u) => format!("{u}"),
                            Value::Int32(i) => format!("{i}"),
                            Value::Float(f) => format!("{f}"),
                            Value::Text(t) => format!("t{}", t.instance_id),
                            other => format!("{other}"),
                        })
                        .collect();
                    format!("a{}[{}]", items.len(), head.join(","))
                }
                _ => "?".into(),
            };
            cols.push(format!("\"{:08x}\":\"{}\"", prop.hash, escape(&desc)));
        }
        rows.push(format!(
            "{{\"g\":{},\"i\":{},\"cols\":{{{}}}}}",
            entry.id.group,
            entry.id.instance,
            cols.join(",")
        ));
    }

    let mut f = std::fs::File::create(&out).expect("create out");
    writeln!(f, "[{}]", rows.join(",\n")).unwrap();
    println!(
        "groups={:?} entries={} -> {out}",
        groups.iter().map(|g| format!("0x{g:08X}")).collect::<Vec<_>>(),
        rows.len()
    );
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
