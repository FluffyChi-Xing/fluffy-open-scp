//! 区域 ↔ 城市组 ↔ 地块表 三方配对（显式引用链），并识别 5城+1伟工 的白水谷。仅开发用。
//!
//! 链路：城市单条 group 的 36B property（0xC1949C4D Key）显式引用区域 desc
//! （00B1B104:<region>:51E7A18D）；地块表（instance 2B9C480C 多份）的
//! uint32 数组（0x16B7B1EF）= 城市组 id → 用交集给区域选表；
//! 模板 key（0x9F2F9B65）区分普通地块 / 伟大工程位。
//!
//! 用法：region_bind <package>
use std::collections::HashMap;

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let p = dbpf::Package::open(&path).unwrap();

    // 1. 城市 group → region（36B property）
    let mut city_region: HashMap<u32, u32> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 {
            continue;
        }
        let Ok(pf) = sc_properties::PropertyFile::parse(&p.read(e).unwrap()) else { continue };
        if let Some(sc_properties::Property { kind: sc_properties::Kind::Scalar(sc_properties::Value::Key(k)), .. }) = pf.get(0xC194_9C4D) {
            if k.instance == 0x51E7_A18D {
                city_region.insert(e.id.group, k.group);
            }
        }
    }
    // region → 城市组集合
    let mut region_cities: HashMap<u32, Vec<u32>> = HashMap::new();
    for (c, r) in &city_region {
        region_cities.entry(*r).or_default().push(*c);
    }

    // 2. 地块表（instance 2B9C480C 多份）：ids + 模板 keys
    let mut tables: Vec<(u32, Vec<u32>, Vec<u32>)> = Vec::new(); // (group, ids, templates)
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x2B9C_480C {
            continue;
        }
        let Ok(pt) = sc_properties::PropertyFile::parse(&p.read(e).unwrap()) else { continue };
        let ids: Vec<u32> = match pt.get(0x16B7_B1EF) {
            Some(sc_properties::Property { kind: sc_properties::Kind::Array(vs), .. }) => vs
                .iter()
                .filter_map(|v| match v { sc_properties::Value::UInt32(x) => Some(*x), _ => None })
                .collect(),
            _ => continue,
        };
        let tpl: Vec<u32> = match pt.get(0x9F2F_9B65) {
            Some(sc_properties::Property { kind: sc_properties::Kind::Array(vs), .. }) => vs
                .iter()
                .filter_map(|v| match v { sc_properties::Value::Key(k) => Some(k.instance), _ => None })
                .collect(),
            _ => vec![],
        };
        tables.push((e.id.group, ids, tpl));
    }
    for (g, ids, tpl) in &tables {
        let mut tpl_counts: HashMap<u32, usize> = HashMap::new();
        for t in tpl { *tpl_counts.entry(*t).or_default() += 1; }
        println!(
            "table {g:08X}: {} ids, templates {:?}",
            ids.len(),
            tpl_counts.iter().map(|(k, n)| format!("{k:08X}×{n}")).collect::<Vec<_>>()
        );
    }

    // 3. 区域 ↔ 表：城市 id 交集
    let mut regions: Vec<u32> = region_cities.keys().copied().collect();
    regions.sort();
    println!("regions with cities: {}", regions.len());
    for r in &regions {
        let cities = &region_cities[r];
        let set: std::collections::HashSet<u32> = cities.iter().copied().collect();
        let mut best: Option<(usize, u32, usize)> = None; // (overlap, table group, table len)
        for (g, ids, _) in &tables {
            let ov = ids.iter().filter(|i| set.contains(i)).count();
            if best.map(|(b, _, _)| ov > b).unwrap_or(true) {
                best = Some((ov, *g, ids.len()));
            }
        }
        if let Some((ov, tg, tl)) = best {
            println!(
                "region {r:08X}: cities={} best-table {tg:08X} overlap {ov}/{} (table {} plots)",
                cities.len(), tl, tl
            );
        }
    }
}
