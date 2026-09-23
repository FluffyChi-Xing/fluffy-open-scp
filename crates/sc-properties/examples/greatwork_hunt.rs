//! 探针：定位区域 desc 未释读 Key 1437550736 + 全部地块表转储（Q4）。仅开发用。
fn main() {
    let target = 1437550736u32;
    println!("target = {target:08X}");
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    for e in p.entries() {
        if e.id.instance == target {
            let d = p.read(e).unwrap_or_default();
            println!("RT0 命中 TGI {:08X}:{:08X}:{:08X} {} B", e.id.type_id, e.id.group, e.id.instance, d.len());
            if let Ok(pf) = sc_properties::PropertyFile::parse(&d) {
                println!("  property {} 键:", pf.values.len());
                for pr in &pf.values {
                    let v = match &pr.kind {
                        sc_properties::Kind::Scalar(v) => format!("{v:?}"),
                        sc_properties::Kind::Array(vs) => {
                            let s: Vec<String> = vs.iter().take(5).map(|x| format!("{x:?}")).collect();
                            format!("[{};{}]", s.join(","), vs.len())
                        }
                        sc_properties::Kind::Empty => "(empty)".into(),
                    };
                    println!("    {:08X}: {}", pr.hash, v);
                }
            }
        }
    }
    // 全部地块表
    println!("-- 全部 2B9C480C 地块表 --");
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x2B9C_480C { continue; }
        let d = p.read(e).unwrap_or_default();
        let Ok(pf) = sc_properties::PropertyFile::parse(&d) else { continue };
        let ids: Vec<u32> = pf.get(0x16B7_B1EF).and_then(|pr| match &pr.kind {
            sc_properties::Kind::Array(vs) => Some(vs.iter().filter_map(|v| match v { sc_properties::Value::UInt32(x) => Some(*x), _ => None }).collect()),
            _ => None,
        }).unwrap_or_default();
        let pos: Vec<(f32, f32)> = pf.get(0xF01D_E4B1).and_then(|pr| match &pr.kind {
            sc_properties::Kind::Array(vs) => Some(vs.iter().filter_map(|v| match v { sc_properties::Value::Vector2(v) => Some((v[0], v[1])), _ => None }).collect()),
            _ => None,
        }).unwrap_or_default();
        println!("表 group={:08X} ids={} positions={}", e.id.group, ids.len(), pos.len());
        for (i, id) in ids.iter().enumerate() {
            let pv = pos.get(i).map(|(x, y)| format!("({x:.0},{y:.0})")).unwrap_or_else(|| "-".into());
            println!("   [{i}] {id:08X} {pv}");
        }
    }
}
