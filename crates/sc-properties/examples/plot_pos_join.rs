//! 探针：BEAF0510 地块表（names+positions）与 Confluence 模板城市坐标精确配对。仅开发用。
fn main() {
    let rt0 = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    // BEAF0510 的城市组 id 集
    let mut ids: Vec<u32> = Vec::new();
    for e in rt0.entries() {
        if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 { continue; }
        let Ok(pf) = sc_properties::PropertyFile::parse(&rt0.read(e).unwrap()) else { continue };
        if let Some(sc_properties::Property { kind: sc_properties::Kind::Scalar(sc_properties::Value::Key(k)), .. }) = pf.get(0xC194_9C4D) {
            if k.instance == 0x51E7_A18D && k.group == 0xBEAF_0510 { ids.push(e.id.group); }
        }
    }
    let set: std::collections::HashSet<u32> = ids.iter().copied().collect();
    // 选重叠最大的地块表，输出 names+ids+positions
    for e in rt0.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x2B9C_480C { continue; }
        let Ok(pt) = sc_properties::PropertyFile::parse(&rt0.read(e).unwrap()) else { continue };
        let tids: Vec<u32> = pt.get(0x16B7_B1EF).and_then(|p| match &p.kind {
            sc_properties::Kind::Array(vs) => Some(vs.iter().filter_map(|v| match v { sc_properties::Value::UInt32(x) => Some(*x), _ => None }).collect()), _ => None }).unwrap_or_default();
        let ov = tids.iter().filter(|i| set.contains(i)).count();
        if ov < 3 { continue; }
        let names: Vec<String> = pt.get(0x0543_BC96).and_then(|p| match &p.kind {
            sc_properties::Kind::Array(vs) => Some(vs.iter().filter_map(|v| match v { sc_properties::Value::String8(s) => Some(s.clone()), _ => None }).collect()), _ => None }).unwrap_or_default();
        let pos: Vec<(f32, f32)> = pt.get(0xF01D_E4B1).and_then(|p| match &p.kind {
            sc_properties::Kind::Array(vs) => Some(vs.iter().filter_map(|v| match v { sc_properties::Value::Vector2(v) => Some((v[0], v[1])), _ => None }).collect()), _ => None }).unwrap_or_default();
        println!("表 {:08X} 重叠 {}/{}", e.id.group, ov, tids.len());
        for i in 0..tids.len() {
            println!("  [{i}] uid={:?} id={:08X} pos={:?}", names.get(i), tids[i], pos.get(i));
        }
    }
}
