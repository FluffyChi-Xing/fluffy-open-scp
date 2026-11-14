//! 一次性诊断：dump 任意 .package 内 0x00B1B104 property 资源的完整内容。
//! 用法：`cargo run -p sc-properties --example nighthouse_props -- <pkg>`

use dbpf::Package;
use sc_properties::{PropertyFile, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .expect("usage: nighthouse_props <pkg>");
    let pkg = Package::open(&path)?;
    for e in pkg.entries().iter().filter(|e| e.id.type_id == 0x00B1B104) {
        let data = pkg.read(e)?;
        println!(
            "\n=== PROP {:08X}:{:08X}:{:08X} ({} bytes) ===",
            e.id.type_id,
            e.id.group,
            e.id.instance,
            data.len()
        );
        let file = PropertyFile::parse(&data)?;
        for p in &file.values {
            match &p.kind {
                sc_properties::Kind::Scalar(v) => println!("  {:08X}: {}", p.hash, v),
                sc_properties::Kind::Array(vs) => {
                    println!("  {:08X}: [{} items]", p.hash, vs.len());
                    for (i, v) in vs.iter().enumerate().take(40) {
                        println!("    [{i}] {}", render(v));
                    }
                    if vs.len() > 40 {
                        println!("    … {} more", vs.len() - 40);
                    }
                }
                sc_properties::Kind::Empty => println!("  {:08X}: <empty>", p.hash),
            }
        }
    }
    Ok(())
}

fn render(v: &Value) -> String {
    match v {
        Value::Key(k) => format!("Key {:08X}:{:08X}:{:08X}", k.type_id, k.group, k.instance),
        other => format!("{other}"),
    }
}
