fn main() {
    let base = std::env::args().nth(1).expect("SimCityData");
    let rt0 =
        dbpf::Package::open(std::path::Path::new(&base).join("SimCity_RegionTerrain0.package"))
            .unwrap();
    let ep1 = dbpf::Package::open(std::path::Path::new(&base).join("SimCityDataEP1.package")).ok();
    let game = dbpf::Package::open(std::path::Path::new(&base).join("SimCity_Game.package")).ok();
    let locale =
        dbpf::Package::open(std::path::Path::new(&base).join("Locale/zh-tw/Data.package")).ok();
    for group in [0xC2A9_C48Fu32, 0xBEAF_0510] {
        let r = sc_properties::region_3d::region_3d(
            &rt0,
            group,
            ep1.as_ref(),
            game.as_ref(),
            locale.as_ref(),
        )
        .unwrap();
        println!("group={group:08X} brushes={}", r.brushes.len());
        for (name, stamps) in r.brushes {
            println!("  {name} {} {:?}", stamps.len(), stamps.first());
        }
    }
}
