//! 探针：城市组 36B property 的 0x1B53D525 是否= uid。仅开发用。
fn main() {
    let rt0 = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    for g in [0xBEAF_0510u32, 0xBC35_7A2B] {
        println!("== 区域 {g:08X}");
        for e in rt0.entries() {
            if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 { continue; }
            let Ok(pf) = sc_properties::PropertyFile::parse(&rt0.read(e).unwrap()) else { continue };
            let Some(sc_properties::Property { kind: sc_properties::Kind::Scalar(sc_properties::Value::Key(k)), .. }) = pf.get(0xC194_9C4D) else { continue };
            if !(k.instance == 0x51E7_A18D && k.group == g) { continue }
            let n = pf.get(0x1B53_D525).and_then(|p| match &p.kind {
                sc_properties::Kind::Scalar(sc_properties::Value::Int32(v)) => Some(*v), _ => None });
            println!("  城市组 {:08X} int32={:?}", e.id.group, n);
        }
    }
}
