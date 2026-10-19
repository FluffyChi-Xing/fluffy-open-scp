//! 探针：转储 coalEcoMapBrushes 全部键值。仅开发用。
fn main() {
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.group != 0xBEAF_0510 || e.id.instance != 0xAD03_BA05 { continue; }
        let d = p.read(e).unwrap();
        let pf = sc_properties::PropertyFile::parse(&d).unwrap();
        for pr in &pf.values {
            let v = match &pr.kind {
                sc_properties::Kind::Scalar(v) => format!("{v:?}"),
                sc_properties::Kind::Array(vs) => {
                    let s: Vec<String> = vs.iter().take(4).map(|x| format!("{x:?}")).collect();
                    format!("[{};{}]", s.join(","), vs.len())
                }
                sc_properties::Kind::Empty => "(empty)".into(),
            };
            println!("{:08X}: {}", pr.hash, v);
        }
    }
}
