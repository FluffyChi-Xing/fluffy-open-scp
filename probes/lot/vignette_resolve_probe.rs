//! 探针：定位 lot 的 vignette 列表(0x0D2E8A55) 指向的定义体。
//! 输出：每个 lot 的列表 key（type/group/instance），以及全包所有「含 0x0D802215
//! （actor startPoint）的属性文件」的 TGI，便于在 Python 侧做交集/分组统计。仅开发用。
//!
//! 用法：cargo run -p probes --example vignette_resolve_probe -- <package> [max-lots]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};

const SPAWNER_IDS: u32 = 0x0E1B_AC61;
const VIGNETTE_LIST: u32 = 0x0D2E_8A55;
const ACTOR_START: u32 = 0x0D80_2215;

fn keys_with_group(file: &PropertyFile, col: u32) -> Vec<(u32, u32, u32)> {
    let Some(p) = file.get(col) else {
        return Vec::new();
    };
    match &p.kind {
        Kind::Array(items) => items
            .iter()
            .filter_map(|v| match v {
                Value::Key(k) => Some((k.type_id, k.group, k.instance)),
                _ => None,
            })
            .collect(),
        Kind::Scalar(Value::Key(k)) => vec![(k.type_id, k.group, k.instance)],
        _ => Vec::new(),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: vignette_resolve_probe <package> [max-lots]");
    let max_lots: usize = args.next().and_then(|v| v.parse().ok()).unwrap_or(6);
    let pkg = Package::open(&path).expect("open package");

    let mut defs: Vec<String> = Vec::new();
    let mut lots: Vec<(u32, Vec<(u32, u32, u32)>)> = Vec::new();
    for entry in pkg.entries() {
        if entry.id.type_id != 0x00B1_B104 {
            continue;
        }
        let Ok(data) = pkg.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else {
            continue;
        };
        if file.get(ACTOR_START).is_some() {
            defs.push(format!(
                "{{\"t\":{},\"g\":{},\"i\":{}}}",
                entry.id.type_id, entry.id.group, entry.id.instance
            ));
        }
        if file.get(SPAWNER_IDS).is_some() {
            let k = keys_with_group(&file, VIGNETTE_LIST);
            if !k.is_empty() {
                lots.push((entry.id.instance, k));
            }
        }
    }
    println!("--- defs_with_startPoint: {}", defs.len());
    let mut by_group: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for d in &defs {
        let g: u32 = d
            .split("\"g\":")
            .nth(1)
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        *by_group.entry(g).or_default() += 1;
    }
    for (g, n) in &by_group {
        println!("    group 0x{g:08X} x{n}");
    }
    println!("--- lots_with_vignette_list: {}", lots.len());
    for (lot, ks) in lots.iter().take(max_lots) {
        println!("lot 0x{lot:08X}");
        for (t, g, i) in ks.iter().take(4) {
            println!("    key t=0x{t:08X} g=0x{g:08X} i=0x{i:08X}");
        }
    }
    // 落盘供 Python 交集
    let mut out = String::from("{\"defs\":[");
    out.push_str(&defs.join(","));
    out.push_str("],\"lots\":[");
    let lots_js: Vec<String> = lots
        .iter()
        .map(|(lot, ks)| {
            let kk: Vec<String> = ks
                .iter()
                .map(|(t, g, i)| format!("{{\"t\":{t},\"g\":{g},\"i\":{i}}}"))
                .collect();
            format!("{{\"lot\":{lot},\"keys\":[{}]}}", kk.join(","))
        })
        .collect();
    out.push_str(&lots_js.join(","));
    out.push_str("]}");
    std::fs::write("tmp/vignette_resolve.json", out).expect("write");
    println!("written tmp/vignette_resolve.json");
}
