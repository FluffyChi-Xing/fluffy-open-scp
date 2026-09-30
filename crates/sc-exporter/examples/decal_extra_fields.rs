//! 冒烟工具：打印 lot decal 单元的第二 Key(0x0D109090)/Int32(0x0D1090A0)/Float(0x0D1090B0)。
//! 用法：cargo run -p sc-exporter --release --example decal_extra_fields -- <lot_instance_hex> <pkg> [...]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lot = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    let packages: Vec<Package> = args
        .iter()
        .skip(1)
        .map(|p| Package::open(p).unwrap_or_else(|e| panic!("open {p}: {e}")))
        .collect();
    for package in &packages {
        let Some(entry) = package.entries().iter().find(|e| e.id.type_id == PROPERTY_TYPE && e.id.instance == lot).cloned() else { continue };
        let data = package.read(&entry).unwrap_or_default();
        let file = PropertyFile::parse(&data).expect("parse");
        for category in 0..3u32 {
            let Some(idp) = file.get(0x0D10_9050 + category) else { continue };
            let Some(Kind::Array(ids)) = Some(&idp.kind) else { continue };
            let key2 = file.get(0x0D10_9090 + category).and_then(|p| p.array().map(|v| v.to_vec()));
            let ints = file.get(0x0D10_90A0 + category).and_then(|p| p.array().map(|v| v.to_vec()));
            let floats = file.get(0x0D10_90B0 + category).and_then(|p| p.array().map(|v| v.to_vec()));
            println!("== category {category}: {} decals", ids.len());
            for (i, v) in ids.iter().enumerate() {
                let id = match v { Value::Key(k) => format!("{:08X}", k.instance), o => format!("{o:?}") };
                let k2 = key2.as_ref().and_then(|a| a.get(i)).map(|v| match v {
                    Value::Key(k) => format!("T{:08X} G{:08X} I{:08X}", k.type_id, k.group, k.instance),
                    o => format!("{o:?}"),
                }).unwrap_or_else(|| "-".into());
                let int = ints.as_ref().and_then(|a| a.get(i)).map(|v| format!("{v:?}")).unwrap_or_else(|| "-".into());
                let flt = floats.as_ref().and_then(|a| a.get(i)).map(|v| format!("{v:?}")).unwrap_or_else(|| "-".into());
                println!("  [{i}] id={id} key2={k2} int32={int} float={flt}");
            }
        }
        return;
    }
    println!("lot not found");
}
