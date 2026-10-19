//! 探针：区域 desc 14 个 Key → 组内 property 名解析（确认有无独立路网资源）。仅开发用。
fn main() {
    let rt0 = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    let game = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_Game.package").unwrap();
    let group = 0xBEAF_0510u32;
    // 组内全部 property 名（0x00B2CCCA）
    let mut names: std::collections::HashMap<u32, String> = Default::default();
    for e in rt0.entries() {
        if e.id.type_id == 0x00B1_B104 && e.id.group == group {
            if let Ok(pf) = sc_properties::PropertyFile::parse(&rt0.read(e).unwrap()) {
                if let Some(sc_properties::Property { kind: sc_properties::Kind::Scalar(sc_properties::Value::String8(s)), .. }) = pf.get(0x00B2_CCCA) {
                    names.insert(e.id.instance, s.clone());
                }
            }
        }
    }
    // 区域 desc 的全部 Key
    for e in rt0.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.group != group || e.id.instance != 0x51E7_A18D { continue; }
        let pf = sc_properties::PropertyFile::parse(&rt0.read(e).unwrap()).unwrap();
        for pr in &pf.values {
            if let sc_properties::Kind::Scalar(sc_properties::Value::Key(k)) = &pr.kind {
                let local = names.get(&k.instance).cloned();
                // 跨包搜该 instance
                let mut ext = String::new();
                for pkg in [&rt0, &game] {
                    for e2 in pkg.entries() {
                        if e2.id.instance == k.instance && !(e2.id.type_id == 0x00B1_B104 && e2.id.group == group) {
                            ext = format!(" {:08X}:{:08X}:{:08X} @{}", e2.id.type_id, e2.id.group, e2.id.instance,
                                if std::ptr::eq(pkg, &game) { "Game" } else { "RT0" });
                            break;
                        }
                    }
                    if !ext.is_empty() { break; }
                }
                println!("desc key {:08X} inst {:08X} -> {:?}{}", pr.hash, k.instance, local, ext);
            }
        }
    }
}
