//! 探针：定位 EcoGame 的 transport entry 表（master 上的 0x0A30A558 KeyArray → group 列表），
//! 逐条 dump transport entry 的关键列：handler / speed / 图形 key(0x0D8C29C3) /
//! transportType(0x0E27B9AF) / 活动文案(0x0E4F02C2,0x0E4F02CE,0x0ECB2216,0x0ECB21EB)，
//! 并把 key instance 反查 s3db 名字。仅开发用。
//!
//! 用法：cargo run -p probes --example simm_transport_probe -- <package> [s3db]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: simm_transport_probe <package> [s3db]");
    let s3db = args.next();
    let package = Package::open(&path).expect("open package");

    let names: BTreeMap<u32, String> = match s3db {
        Some(p) => load_names(&p),
        None => BTreeMap::new(),
    };

    // 1) 找 master：含 0x0A30A558 的属性文件
    let mut transport_groups: Vec<u32> = Vec::new();
    let mut masters: Vec<String> = Vec::new();
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
                        transport_groups.push(k.group);
                    }
                }
                masters.push(format!(
                    "t=0x{:08X} g=0x{:08X} i=0x{:08X}",
                    entry.id.type_id, entry.id.group, entry.id.instance
                ));
            }
        }
    }
    transport_groups.sort_unstable();
    transport_groups.dedup();
    println!("--- masters with 0x0A30A558: {}", masters.len());
    for m in masters.iter().take(8) {
        println!("    {m}");
    }
    println!("--- transport groups: {}", transport_groups.len());
    for g in transport_groups.iter().take(20) {
        println!("    group 0x{g:08X}");
    }

    // 2) 枚举这些 group 下的所有属性文件 = transport entries
    let set: std::collections::BTreeSet<u32> = transport_groups.iter().copied().collect();
    let mut shown = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 || !set.contains(&entry.id.group) {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        shown += 1;
        let gfx: Option<u32> = file.get(0x0D8C_29C3).and_then(|p| key_instance(p));
        let name = names
            .get(&entry.id.instance)
            .cloned()
            .unwrap_or_else(|| "?".into());
        let gfx_name = gfx
            .and_then(|g| names.get(&g).cloned())
            .unwrap_or_else(|| "?".into());
        let handler = file.get(0x0A31_A734).and_then(|p| key_instance(p));
        let handler_name = handler
            .and_then(|h| names.get(&h).cloned())
            .unwrap_or_else(|| "?".into());
        let speed = file.get(0x0A2F_86E4).and_then(|p| float_of(p));
        println!(
            "\nentry g=0x{:08X} i=0x{:08X} {name}\n   gfxKey=0x{:08X} {gfx_name}\n   handler=0x{:08X} {handler_name}  speed={:?}",
            entry.id.group,
            entry.id.instance,
            gfx.unwrap_or(0),
            handler.unwrap_or(0),
            speed
        );
        for col in [
            0x0E27_B9AFu32,
            0x0E4F_02C2,
            0x0E4F_02CE,
            0x0ECB_2216,
            0x0ECB_21EB,
            0x0ECB_21EA,
            0x0ECB_2204,
            0x0E4F_02A9,
        ] {
            if let Some(p) = file.get(col) {
                let desc = describe(p, &names);
                println!("   col 0x{col:08X} => {desc}");
            }
        }
        if shown >= 400 {
            println!("... truncated");
            break;
        }
    }
    println!("\n--- transport entries seen: {shown}");
}

fn key_instance(p: &sc_properties::Property) -> Option<u32> {
    match &p.kind {
        Kind::Scalar(Value::Key(k)) => Some(k.instance),
        Kind::Scalar(Value::UInt32(u)) => Some(*u),
        _ => None,
    }
}

fn float_of(p: &sc_properties::Property) -> Option<f32> {
    match &p.kind {
        Kind::Scalar(Value::Float(f)) => Some(*f),
        _ => None,
    }
}

fn describe(p: &sc_properties::Property, names: &BTreeMap<u32, String>) -> String {
    fn one(v: &Value, names: &BTreeMap<u32, String>) -> String {
        match v {
            Value::Key(k) => format!("Key(0x{:08X} {})", k.instance, names.get(&k.instance).cloned().unwrap_or_else(|| "?".into())),
            Value::UInt32(u) => format!("U32(0x{u:08X} {})", names.get(u).cloned().unwrap_or_else(|| "?".into())),
            Value::Text(t) => format!("Text(t=0x{:08X} i=0x{:08X})", t.table_id, t.instance_id),
            Value::String8(s) | Value::String16(s) => format!("Str({s})"),
            Value::Float(f) => format!("f32({f})"),
            Value::Int32(i) => format!("i32({i})"),
            other => format!("{other}"),
        }
    }
    match &p.kind {
        Kind::Scalar(v) => one(v, names),
        Kind::Array(items) => {
            let head: Vec<String> = items.iter().take(6).map(|v| one(v, names)).collect();
            format!("[{}] len={}", head.join(", "), items.len())
        }
        Kind::Empty => "empty".into(),
    }
}

fn load_names(path: &str) -> BTreeMap<u32, String> {
    let mut out = BTreeMap::new();
    if let Ok(reg) = sc_registry::Registry::open(path) {
        for (id, rec) in reg.instances() {
            if !rec.name.is_empty() {
                out.insert(*id, rec.name.clone());
            }
        }
    }
    out
}
