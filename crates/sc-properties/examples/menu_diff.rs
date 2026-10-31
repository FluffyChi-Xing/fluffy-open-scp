//! 一次性诊断：对拍任意 .package 内 property 资源的线上格式（hash / 类型码 /
//! flags / 步长 / 值摘要）与 locale JSON 原文。用于已验证 overlay 与新构建包
//! 的字节级对照。
//! 用法：`cargo run -p sc-properties --example menu_diff -- <pkg>...`

use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in std::env::args().skip(1) {
        println!("\n================ {path} ================");
        let pkg = Package::open(&path)?;
        for e in pkg.entries() {
            match e.id.type_id {
                0x00B1B104 => dump_property(&pkg, e),
                0x0A98EAF0 => {
                    let data = pkg.read(e)?;
                    println!(
                        "JSON {:08X}:{:08X}:{:08X} ({} bytes): {}",
                        e.id.type_id,
                        e.id.group,
                        e.id.instance,
                        data.len(),
                        String::from_utf8_lossy(&data)
                    );
                }
                0x2F7D0004 | 0x3F8662EA => {
                    let data = pkg.read(e)?;
                    println!(
                        "IMG  {:08X}:{:08X}:{:08X} ({} bytes) head={:02x?}",
                        e.id.type_id,
                        e.id.group,
                        e.id.instance,
                        data.len(),
                        &data[..data.len().min(8)]
                    );
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn dump_property(pkg: &Package, e: &dbpf::IndexEntry) {
    let data = match pkg.read(e) {
        Ok(data) => data,
        Err(err) => {
            println!("PROP {:08X} read error {err}", e.id.instance);
            return;
        }
    };
    println!(
        "PROP {:08X}:{:08X}:{:08X} ({} bytes)",
        e.id.type_id,
        e.id.group,
        e.id.instance,
        data.len()
    );
    let file = match PropertyFile::parse(&data) {
        Ok(file) => file,
        Err(err) => {
            println!("  parse FAILED: {err}");
            return;
        }
    };
    for p in &file.values {
        let flags = p.encoding.flags;
        let item = p
            .encoding
            .array_item_size
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".into());
        let summary = match &p.kind {
            Kind::Scalar(v) => render(v),
            Kind::Array(vs) => format!(
                "[{} items] {}",
                vs.len(),
                vs.iter().take(3).map(render).collect::<Vec<_>>().join("; ")
            ),
            Kind::Empty => "<empty>".into(),
        };
        println!(
            "  {:08X} type={} flags={:#06x} item={item}: {summary}",
            p.hash,
            p.prop_type.file_type(),
            flags
        );
    }
}

fn render(v: &Value) -> String {
    match v {
        Value::Key(k) => format!(
            "Key{T:08X}:{G:08X}:{I:08X}",
            T = k.type_id,
            G = k.group,
            I = k.instance
        ),
        Value::Text(t) => format!("Text(tbl={:08X},id={:08X})", t.table_id, t.instance_id),
        Value::Int32(i) => format!("i32 {i}"),
        Value::UInt32(u) => format!("u32 {u}"),
        Value::Bool(b) => format!("bool {b}"),
        Value::Float(f) => format!("f32 {f}"),
        other => format!("{other}"),
    }
}
