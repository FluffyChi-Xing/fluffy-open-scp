//! 探针：region_list_named 真名链验证。仅开发用。
fn main() {
    let base = "D:/ea-games/SimCity/SimCityData";
    let rt0 = dbpf::Package::open(format!("{base}/SimCity_RegionTerrain0.package")).unwrap();
    let ep1 = dbpf::Package::open(format!("{base}/SimCityDataEP1.package")).ok();
    let game = dbpf::Package::open(format!("{base}/SimCity_Game.package")).ok();
    let locale = dbpf::Package::open(format!("{base}/Locale/zh-tw/Data.package")).ok();
    for r in sc_properties::region_3d::region_list_named(&rt0, ep1.as_ref(), game.as_ref(), locale.as_ref()) {
        println!("{:08X} [{}] zh={:?} en={:?}", r.group, r.plot_count, r.display_name, r.display_name_en);
    }
}
