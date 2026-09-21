//! 地图机制调查探针（只读）：
//!   survey <package>              —— 按类型统计条目（数量/字节量/代表 TGI）
//!   dump   <package> <type-hex>   —— 打印某类型全部条目（TGI + 尺寸 + 头部 hex）
//!   props  <package>              —— 全量打印包内 0x00B1B104 property 键值摘要
//! 用于 SimCity_RegionTerrain*.package / 模组覆盖包的地图机制取证。

use dbpf::Package;
use sc_properties::{PropertyFile, Value};
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args[0].as_str() {
        "survey" => survey(&args[1]),
        "dump" => dump(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap()),
        "props" => props(&args[1]),
        // hstats <package> <type-hex> —— u16 高度图逐条 min/max/avg（跳 20 字节头）
        "hstats" => hstats(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap()),
        // extract <package> <instance-hex> <out> —— 按 instance 提取第一条资源
        "extract" => extract(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap(), &args[3]),
        // tilecheck <package> <instA> <instB> —— 检验两张 256×256 u16 高度图的边缘连续性
        "tilecheck" => tilecheck(&args[1], u32::from_str_radix(&args[2].trim_start_matches("0x"), 16).unwrap(), u32::from_str_radix(&args[3].trim_start_matches("0x"), 16).unwrap()),
        other => panic!("unknown subcommand {other}"),
    }
}

fn height_grid(path: &str, instance: u32) -> Option<(dbpf::ResourceId, Vec<u16>)> {
    let package = open(path);
    for entry in package.entries().iter() {
        if entry.id.instance == instance {
            let data = package.read(entry).ok()?;
            let cells: Vec<u16> = data[20..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            return Some((entry.id, cells));
        }
    }
    None
}

/// 相邻 tile 判定：A 右缘 vs B 左缘（及下缘/上缘）平均差显著小于随机边缘差。
fn tilecheck(path: &str, inst_a: u32, inst_b: u32) {
    let (ida, a) = height_grid(path, inst_a).expect("A missing");
    let (idb, b) = height_grid(path, inst_b).expect("B missing");
    let edge = |cells: &[u16], side: &str| -> Vec<u16> {
        (0..256)
            .map(|i| match side {
                "right" => cells[i * 256 + 255],
                "left" => cells[i * 256],
                "bottom" => cells[255 * 256 + i],
                _ => cells[i],
            })
            .collect()
    };
    let diff = |x: &[u16], y: &[u16]| -> f64 {
        x.iter().zip(y).map(|(p, q)| (*p as i64 - *q as i64).unsigned_abs() as u64).sum::<u64>() as f64 / 256.0
    };
    println!("A {:08X}  B {:08X}", ida.instance, idb.instance);
    println!("A.right vs B.left  avg|d| = {:.1}", diff(&edge(&a, "right"), &edge(&b, "left")));
    println!("A.left  vs B.right avg|d| = {:.1}", diff(&edge(&a, "left"), &edge(&b, "right")));
    println!("A.bottom vs B.top   avg|d| = {:.1}", diff(&edge(&a, "bottom"), &edge(&b, "top")));
    println!("A.top vs B.bottom   avg|d| = {:.1}", diff(&edge(&a, "top"), &edge(&b, "bottom")));
    println!("A.right vs A.left(自身对照) avg|d| = {:.1}", diff(&edge(&a, "right"), &edge(&a, "left")));
}

fn hstats(path: &str, type_id: u32) {
    let package = open(path);
    let mut shown = 0usize;
    for entry in package.entries().iter().filter(|e| e.id.type_id == type_id) {
        let Ok(data) = package.read(entry) else { continue };
        if data.len() < 20 || (data.len() - 20) % 2 != 0 { continue; }
        let cells: Vec<u16> = data[20..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let min = cells.iter().min().unwrap();
        let max = cells.iter().max().unwrap();
        let avg = cells.iter().map(|v| *v as u64).sum::<u64>() / cells.len() as u64;
        let nonzero = cells.iter().filter(|v| **v != 0).count();
        println!(
            "{:08X}:{:08X}:{:08X}  cells={} min={min} max={max} avg={avg} nonzero={nonzero}",
            entry.id.type_id, entry.id.group, entry.id.instance, cells.len()
        );
        shown += 1;
        if shown >= 8 { break; }
    }
}

fn extract(path: &str, instance: u32, out: &str) {
    let package = open(path);
    for entry in package.entries().iter() {
        if entry.id.instance == instance {
            let data = package.read(entry).unwrap();
            std::fs::write(out, &data).unwrap();
            println!("extracted {} bytes -> {out}", data.len());
            return;
        }
    }
    panic!("instance {instance:08X} not found");
}

fn open(path: &str) -> Package {
    Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"))
}

fn survey(path: &str) {
    let package = open(path);
    let mut by_type: BTreeMap<u32, (usize, u64, Option<dbpf::ResourceId>)> = BTreeMap::new();
    for entry in package.entries() {
        let slot = by_type.entry(entry.id.type_id).or_insert((0, 0, None));
        slot.0 += 1;
        slot.1 += u64::from(entry.decompressed_size);
        if slot.2.is_none() {
            slot.2 = Some(entry.id);
        }
    }
    println!("== {path} ==");
    let mut rows: Vec<_> = by_type.iter().collect();
    rows.sort_by_key(|(_, (count, size, _))| (*size, *count));
    for (type_id, (count, size, sample)) in rows.iter().rev() {
        println!(
            "type 0x{type_id:08X}  count={count}  bytes={size}  sample={:?}",
            sample.map(|id| format!(
                "{:08X}:{:08X}:{:08X}",
                id.type_id, id.group, id.instance
            )),
        );
    }
}

fn head_hex(data: &[u8], n: usize) -> String {
    data.iter()
        .take(n)
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn dump(path: &str, type_id: u32) {
    let package = open(path);
    for entry in package.entries().iter().filter(|e| e.id.type_id == type_id) {
        let data = package.read(entry).unwrap_or_default();
        println!(
            "{:08X}:{:08X}:{:08X}  stored={} size={}  head={}",
            entry.id.type_id,
            entry.id.group,
            entry.id.instance,
            entry.stored_len(),
            entry.decompressed_size,
            head_hex(&data, 16)
        );
    }
}

fn value_summary(value: &Value) -> String {
    match value {
        Value::UInt32(v) => format!("u32 {v} (0x{v:08X})"),
        Value::Int32(v) => format!("i32 {v}"),
        Value::Float(v) => format!("f32 {v:.4}"),
        Value::Key(k) => format!("key {:08X}:{:08X}:{:08X}", k.type_id, k.group, k.instance),
        Value::Vector2(v) => format!("vec2 {:?}", v),
        Value::Vector3(v) => format!("vec3 {:?}", v),
        other => format!("{other}"),
    }
}

fn props(path: &str) {
    let package = open(path);
    for entry in package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == 0x00B1_B104)
    {
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else {
            println!("-- {:08X}:{:08X}:{:08X} parse failed", entry.id.type_id, entry.id.group, entry.id.instance);
            continue;
        };
        println!(
            "== property {:08X}:{:08X}:{:08X} ({} props) ==",
            entry.id.type_id, entry.id.group, entry.id.instance,
            file.values.len()
        );
        for property in &file.values {
            match &property.kind {
                sc_properties::Kind::Scalar(v) => {
                    println!("  0x{:08X} {} = {}", property.hash, property.prop_type.name(), value_summary(v));
                }
                sc_properties::Kind::Array(values) => {
                    let preview: Vec<String> =
                        values.iter().take(8).map(value_summary).collect();
                    println!(
                        "  0x{:08X} {}[{}] = {}{}",
                        property.hash,
                        property.prop_type.name(),
                        values.len(),
                        preview.join(", "),
                        if values.len() > 8 { ", …" } else { "" }
                    );
                }
                sc_properties::Kind::Empty => println!("  0x{:08X} <empty>", property.hash),
            }
        }
    }
}
