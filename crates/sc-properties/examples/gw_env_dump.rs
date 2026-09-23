//! 探针：environment 属性全文 + JS GreatWorks 常量搜索由外部完成。仅开发用。
fn main() {
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    for g in [0xBEAF_0510u32, 0xD01F_A985] {
        for e in p.entries() {
            if e.id.type_id != 0x00B1_B104 || e.id.group != g || e.id.instance != 0x494F_8678 { continue; }
            let d = p.read(e).unwrap();
            let pf = sc_properties::PropertyFile::parse(&d).unwrap();
            println!("区域 {g:08X} environment ({:08X}) {} B:", e.id.instance, d.len());
            for pr in &pf.values {
                let v = match &pr.kind {
                    sc_properties::Kind::Scalar(v) => format!("{v:?}"),
                    sc_properties::Kind::Array(vs) => {
                        let s: Vec<String> = vs.iter().take(6).map(|x| format!("{x:?}")).collect();
                        format!("[{};{}]", s.join(","), vs.len())
                    }
                    sc_properties::Kind::Empty => "(empty)".into(),
                };
                println!("  {:08X}: {}", pr.hash, v);
            }
        }
    }
}
