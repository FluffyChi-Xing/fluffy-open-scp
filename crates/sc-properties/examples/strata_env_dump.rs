//! 探针：dump 层栈模板（Game 包 D7EF2862 族）+ 区域 desc 全文 + 环境属性表。仅开发用。
fn main() {
    let game = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_Game.package").unwrap();
    println!("== Game 包 D7EF2862 地形层栈模板族 ==");
    let mut n = 0;
    for e in game.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.group != 0xD7EF_2862 { continue; }
        let Ok(d) = game.read(e) else { continue };
        let Ok(pf) = sc_properties::PropertyFile::parse(&d) else { continue };
        // 只打印被地块表引用的 6F9F6AB3 + 前几条
        if e.id.instance != 0x6F9F_6AB3 && n >= 3 { continue; }
        n += 1;
        println!("-- {:08X}:{:08X}:{:08X} {}B", e.id.type_id, e.id.group, e.id.instance, d.len());
        for pr in &pf.values {
            let v = match &pr.kind {
                sc_properties::Kind::Scalar(v) => format!("{v:?}"),
                sc_properties::Kind::Array(vs) => {
                    let s: Vec<String> = vs.iter().take(4).map(|x| format!("{x:?}")).collect();
                    format!("[{};{}]", s.join(","), vs.len())
                }
                sc_properties::Kind::Empty => "(empty)".into(),
            };
            println!("   {:08X}: {}", pr.hash, v);
        }
    }
    // 区域 desc（BEAF0510）全文 —— 找 0xC1949C4D envID
    let rt0 = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    for e in rt0.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.group != 0xBEAF_0510 || e.id.instance != 0x51E7_A18D { continue; }
        let d = rt0.read(e).unwrap();
        let pf = sc_properties::PropertyFile::parse(&d).unwrap();
        println!("== 区域 desc BEAF0510:51E7A18D 全文 ==");
        for pr in &pf.values {
            let v = match &pr.kind {
                sc_properties::Kind::Scalar(v) => format!("{v:?}"),
                sc_properties::Kind::Array(vs) => {
                    let s: Vec<String> = vs.iter().take(4).map(|x| format!("{x:?}")).collect();
                    format!("[{};{}]", s.join(","), vs.len())
                }
                sc_properties::Kind::Empty => "(empty)".into(),
            };
            println!("   {:08X}: {}", pr.hash, v);
        }
    }
}
