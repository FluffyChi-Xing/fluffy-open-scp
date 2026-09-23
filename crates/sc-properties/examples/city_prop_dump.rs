//! 探针：转储全部城市组 36B property（0x1B53D525 int32 + 0xC1949C4D 区域回指）的 int 值。
//! 假设：该 int = 水位面（raw）。仅开发用。
use std::collections::HashMap;

use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};

fn main() {
    let p = Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package")
        .expect("open");
    let mut by_region: HashMap<u32, Vec<(u32, i32, Vec<(u32, String)>)>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 {
            continue;
        }
        let Ok(data) = p.read(e) else { continue };
        let Ok(pf) = PropertyFile::parse(&data) else { continue };
        let Some(Some(region)) = pf.get(0xC194_9C4D).map(|pr| match &pr.kind {
            Kind::Scalar(Value::Key(k)) => Some(k.group),
            _ => None,
        })
        else {
            continue;
        };
        let int_val = pf.get(0x1B53_D525).and_then(|pr| match &pr.kind {
            Kind::Scalar(Value::Int32(v)) => Some(*v),
            _ => None,
        });
        let extra: Vec<(u32, String)> = pf
            .values
            .iter()
            .filter(|pr| pr.hash != 0xC194_9C4D && pr.hash != 0x1B53_D525)
            .map(|pr| (pr.hash, format!("{:?}", pr.kind)))
            .collect();
        by_region
            .entry(region)
            .or_default()
            .push((e.id.group, int_val.unwrap_or(i32::MIN), extra));
    }
    for (region, cities) in &by_region {
        let mut vals: Vec<i32> = cities.iter().map(|c| c.1).collect();
        vals.sort();
        println!(
            "区域 {region:08X}: {} 城，0x1B53D525 值 min={} max={} 全部={:?}",
            cities.len(),
            vals.first().unwrap_or(&0),
            vals.last().unwrap_or(&0),
            vals
        );
        if let Some((g, _, extra)) = cities.first() {
            if !extra.is_empty() {
                println!("  例（{g:08X}）额外键: {extra:?}");
            }
        }
    }
}
