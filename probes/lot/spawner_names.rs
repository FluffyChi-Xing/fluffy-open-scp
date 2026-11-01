//! 探针：把 lot 的 spawner（UnitLocation）表整体导出为 JSON：lot instance →
//! [{ id, instance, pos:[x,y,z], rot markers }]，并统计全包 spawner 名字 id 频次。
//! 供 Python 侧用 FNV-1(lower) 反查名字。仅开发用。
//!
//! 用法：cargo run -p probes --release --example spawner_names -- <package> [out.json]
use dbpf::Package;
use sc_properties::{PropertyFile, Value};
use std::collections::BTreeMap;
use std::io::Write;

const SPAWNER_IDS: u32 = 0x0E1B_AC61;
const SPAWNER_TRANSFORMS: u32 = 0x0E1B_AC62;

#[derive(Default)]
struct Stats {
    lots_with_spawners: usize,
    total_spawners: usize,
    id_freq: BTreeMap<u32, usize>,
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: spawner_names <package> [out.json]");
    let out = std::env::args().nth(2).unwrap_or_else(|| "spawner_names.json".into());
    let package = Package::open(&path).expect("open package");
    let mut stats = Stats::default();
    let mut lots: Vec<String> = Vec::new();

    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 {
            continue;
        }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        let Some(ids) = file.get(SPAWNER_IDS).and_then(|p| p.array()) else {
            continue;
        };
        if ids.is_empty() {
            continue;
        }
        let transforms = file.get(SPAWNER_TRANSFORMS).and_then(|p| p.array());
        stats.lots_with_spawners += 1;
        stats.total_spawners += ids.len();
        let mut items = Vec::new();
        for (i, value) in ids.iter().enumerate() {
            let Value::Key(k) = value else { continue };
            *stats.id_freq.entry(k.instance).or_default() += 1;
            let mut pos = Vec::new();
            if let Some(t) = transforms.and_then(|t| t.get(i)) {
                if let Value::Transform(tf) = t {
                    for v in tf.matrix.iter() {
                        pos.push((v * 1000.0).round() / 1000.0);
                    }
                }
            }
            items.push(format!(
                "{{\"i\":{i},\"id\":{},\"id_hex\":\"0x{:08X}\",\"t\":[{}]}}",
                k.instance,
                k.instance,
                pos.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(",")
            ));
        }
        lots.push(format!(
            "{{\"lot\":{},\"lot_hex\":\"0x{:08X}\",\"spawners\":[{}]}}",
            entry.id.instance,
            entry.id.instance,
            items.join(",")
        ));
    }

    let mut f = std::fs::File::create(&out).expect("create out");
    writeln!(f, "{{\"package\":{path:?},\"lots\":[").unwrap();
    writeln!(f, "{}", lots.join(",\n")).unwrap();
    writeln!(f, "]}}").unwrap();

    println!(
        "lots_with_spawners={} total_spawners={} distinct_ids={}",
        stats.lots_with_spawners,
        stats.total_spawners,
        stats.id_freq.len()
    );
    println!("--- distinct spawner name ids (top 60 by freq)");
    let mut v: Vec<_> = stats.id_freq.iter().collect();
    v.sort_by(|a, b| b.1.cmp(a.1));
    for (id, n) in v.iter().take(60) {
        println!("0x{id:08X} x{n}");
    }
}
