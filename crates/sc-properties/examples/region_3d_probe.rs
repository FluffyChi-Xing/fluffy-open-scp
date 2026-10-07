//! 探针：region_3d 全链实测（高度/地面 PNG + 地块名 + 伟工位）。仅开发用。
fn main() {
    let base = "D:/ea-games/SimCity/SimCityData";
    let rt0 = dbpf::Package::open(format!("{base}/SimCity_RegionTerrain0.package")).unwrap();
    let ep1 = dbpf::Package::open(format!("{base}/SimCityDataEP1.package")).ok();
    let game = dbpf::Package::open(format!("{base}/SimCity_Game.package")).ok();
    let locale = dbpf::Package::open(format!("{base}/Locale/zh-tw/Data.package")).ok();
    let t0 = std::time::Instant::now();
    let r = sc_properties::region_3d::region_3d(&rt0, 0xBEAF_0510, ep1.as_ref(), game.as_ref(), locale.as_ref()).unwrap();
    println!("耗时 {:.1}s", t0.elapsed().as_secs_f32());
    println!("高度 PNG {} B, 地面 PNG {} B, water_z={}, desert={}", r.height_png.len(), r.ground_png.len(), r.water_z, r.desert);
    println!("区域名: {:?} / {:?}", r.display_name, r.display_name_en);
    std::fs::create_dir_all("tmp/r3d").unwrap();
    std::fs::write("tmp/r3d/height.png", &r.height_png).unwrap();
    std::fs::write("tmp/r3d/ground.png", &r.ground_png).unwrap();
    for p in &r.plots {
        println!("  [{}] uid={} ({:.0},{:.0},{:.0}) {:?}", p.kind, p.uid, p.x, p.y, p.z, p.name);
    }
}
