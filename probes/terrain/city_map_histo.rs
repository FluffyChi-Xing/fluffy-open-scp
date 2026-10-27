//! 探针：每城市组的 256² 高度图直方图——海洋格恒定值 = 引擎水位。仅开发用。
use std::collections::HashMap;

use dbpf::Package;

fn main() {
    let p = Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package")
        .expect("open");
    // 城市组：含 F0 条目 + 36B property（回指区域）
    let mut city_region: HashMap<u32, u32> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x00B1_B104 && e.compressed_size <= 60 {
            city_region.insert(e.id.group, 0);
        }
    }
    // 城市组 → 区域（36B property 的 Key.group）
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 || !city_region.contains_key(&e.id.group) {
            continue;
        }
        if let Ok(data) = p.read(e) {
            if let Ok(pf) = sc_properties::PropertyFile::parse(&data) {
                if let Some(sc_properties::Property {
                    kind: sc_properties::Kind::Scalar(sc_properties::Value::Key(k)),
                    ..
                }) = pf.get(0xC194_9C4D)
                {
                    city_region.insert(e.id.group, k.group);
                }
            }
        }
    }
    // 每城市组的 F0 高度图 → 直方图 top 值
    let mut by_region: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id != 0x03E4_21F0 || !city_region.contains_key(&e.id.group) {
            continue;
        }
        let Ok(d) = p.read(e) else { continue };
        if d.len() != 131092 {
            continue;
        }
        let mut hist: HashMap<u16, usize> = HashMap::new();
        for c in d[20..].chunks_exact(2) {
            *hist.entry(u16::from_le_bytes([c[0], c[1]])).or_default() += 1;
        }
        let mut top: Vec<(u16, usize)> = hist.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1));
        by_region.entry(city_region[&e.id.group]).or_default().push(e.id.group);
        if [0xD01F_A985u32, 0xBEAF_0510].contains(&city_region[&e.id.group]) {
            println!(
                "城 {} (区域 {:08X}): top5 {:?}",
                e.id.group,
                city_region[&e.id.group],
                &top[..5.min(top.len())]
            );
        }
    }
    // 汇总：每区域所有城市 top1 值
    for (region, cities) in &by_region {
        println!("区域 {region:08X}: {} 城已扫", cities.len());
    }
}
