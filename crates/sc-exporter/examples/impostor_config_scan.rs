//! 取证探针：impostor class 配置表定位。
//! ① `dump <inst_hex>`：打印该 instance 全部 0x00B1B104 属性键值；
//! ② 缺省：全包扫描含 impostor 配置键族 0x0BD62576-81 的属性表。
//! 用法：cargo run -p sc-exporter --release --example impostor_config_scan -- [dump <inst_hex>] <pkg> [...]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const IMPOSTOR_KEYS: [u32; 11] = [
    0x0BD6_2576, 0x0BD6_2577, 0x0BD6_2578, 0x0BD6_2579, 0x0BD6_257A, 0x0BD6_257B, 0x0BD6_257C,
    0x0BD6_257E, 0x0BD6_2580, 0x0BD6_2581, 0x0DDE_0508,
];

fn summarize(v: &Value) -> String {
    match v {
        Value::UInt32(n) => format!("u32 {n} (0x{n:08X})"),
        Value::Int32(n) => format!("i32 {n}"),
        Value::Float(f) => format!("f32 {f}"),
        Value::Bool(b) => format!("bool {b}"),
        Value::Key(k) => format!("key I {:08X} G {:08X}", k.instance, k.group),
        Value::BoundingBox { min, max } => format!("bbox {min:?}..{max:?}"),
        other => format!("{other}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dump_inst: Option<u32> = None;
    let mut packages: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "dump" {
            dump_inst =
                it.next().map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap());
        } else {
            packages.push(a.clone());
        }
    }
    for path in &packages {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        for e in package.entries().iter().filter(|e| e.id.type_id == PROPERTY_TYPE) {
            if let Some(inst) = dump_inst {
                if e.id.instance != inst {
                    continue;
                }
            }
            let Ok(data) = package.read(e) else { continue };
            let Ok(doc) = PropertyFile::parse(&data) else { continue };
            let hits: Vec<u32> = doc
                .values
                .iter()
                .filter(|p| IMPOSTOR_KEYS.contains(&p.hash))
                .map(|p| p.hash)
                .collect();
            if let Some(inst) = dump_inst {
                println!("== I {inst:08X} G {:08X} ({} keys)", e.id.group, doc.values.len());
                for p in &doc.values {
                    match &p.kind {
                        Kind::Scalar(v) => println!("   {:08X}: {}", p.hash, summarize(v)),
                        Kind::Array(vals) => println!(
                            "   {:08X}: [{}] {}",
                            p.hash,
                            vals.len(),
                            vals.iter().map(summarize).collect::<Vec<_>>().join(" | ")
                        ),
                        Kind::Empty => println!("   {:08X}: <empty>", p.hash),
                    }
                }
            } else if !hits.is_empty() {
                println!(
                    "{path}: I {:08X} G {:08X} {}B 命中 impostor 配置键 {}",
                    e.id.instance,
                    e.id.group,
                    data.len(),
                    hits.iter().map(|h| format!("{h:08X}")).collect::<Vec<_>>().join(",")
                );
            }
        }
    }
}
