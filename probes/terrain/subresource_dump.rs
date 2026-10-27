//! 探针：区域子资源全清单——名称、画刷 Key、变换、目标 map、分辨率。
//! 回答 Q2：资源图层（coal/ore/oil/watertable…）如何定义、如何落到地图。仅开发用。
use std::collections::HashMap;

use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};

fn main() {
    let p = Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package")
        .unwrap();
    for group in [0xBEAF_0510u32, 0xD01F_A985] {
        println!("\n====== 区域 {:08X} ======", group);
        for e in p.entries() {
            if e.id.type_id != 0x00B1_B104 || e.id.group != group {
                continue;
            }
            let Ok(data) = p.read(e) else { continue };
            let Ok(pf) = PropertyFile::parse(&data) else { continue };
            let Some(Some(name)) = pf.values.iter().find(|pr| pr.hash == 0x00B2_CCCA).map(|pr| match &pr.kind {
                Kind::Scalar(Value::String8(s)) => Some(s.clone()),
                _ => None,
            }) else { continue };
            if name == "region" {
                continue;
            }
            // 汇总该子资源的关键键
            let keys: Vec<u32> = pf.values.iter().map(|pr| pr.hash).collect();
            let map_target = pf.get(0x0DBA_3A9C).and_then(|pr| match &pr.kind {
                Kind::Scalar(Value::String8(s)) => Some(s.clone()),
                _ => None,
            });
            let res = pf.get(0x0DC0_97E3).and_then(|pr| match &pr.kind {
                Kind::Scalar(Value::UInt32(v)) => Some(*v),
                _ => None,
            });
            let brush_keys = pf.get(0x02A9_07B5).and_then(|pr| match &pr.kind {
                Kind::Array(vs) => Some(vs.iter().filter_map(|v| match v {
                    Value::Key(k) => Some(format!("{:08X}", k.instance)),
                    _ => None,
                }).collect::<Vec<_>>()),
                _ => None,
            });
            let transforms = pf.get(0x02A9_07B6).and_then(|pr| match &pr.kind {
                Kind::Array(vs) => Some(vs.iter().filter_map(|v| match v {
                    Value::Transform(t) if t.matrix.len() >= 11 => {
                        Some(format!("({:.0},{:.0})", t.matrix[9], t.matrix[10]))
                    }
                    _ => None,
                }).collect::<Vec<_>>()),
                _ => None,
            });
            let strengths = pf.get(0x03A2_3F97).and_then(|pr| match &pr.kind {
                Kind::Array(vs) => Some(vs.iter().filter_map(|v| match v {
                    Value::Float(f) => Some(format!("{f}")),
                    _ => None,
                }).collect::<Vec<_>>()),
                _ => None,
            });
            println!(
                "  {:<28} inst={:08X} {}B 键数={} 目标map={:?} 分辨率={:?}",
                name,
                e.id.instance,
                data.len(),
                keys.len(),
                map_target,
                res
            );
            if let Some(bk) = &brush_keys {
                println!("      stampKey: {:?}", bk);
            }
            if let Some(tr) = &transforms {
                println!("      transforms[{}]: {:?}", tr.len(), tr);
            }
            if let Some(st) = &strengths {
                println!("      strengths: {:?}", st);
            }
        }
    }
    // 全包按 instance 找这些 stamp 资源是否存在
    println!("\n====== stamp instance 存在性（RT0 内）======");
    let mut wanted: HashMap<u32, Vec<&str>> = HashMap::new();
    let _ = &mut wanted;
    let mut all_insts: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for e in p.entries() {
        all_insts.insert(e.id.instance);
    }
    println!("RT0 全部唯一 instance 数: {}", all_insts.len());
}
