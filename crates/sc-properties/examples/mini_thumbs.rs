//! 探针：渲染全部未命名区域的地形缩略图（供目视消歧）。仅开发用。
fn main() {
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package")
        .unwrap();
    std::fs::create_dir_all("tmp/region_preview").unwrap();
    for group in [0xC041_82E4u32, 0xBC35_7A2B, 0xB12D_E348, 0xE41A_82B8, 0xA0B6_0DDE, 0xE018_3D94, 0xDB25_018C] {
        let t0 = std::time::Instant::now();
        let out = sc_properties::region_map::render_region_png(&p, group, None).unwrap();
        let img = image::load_from_memory(&out.png).unwrap().to_rgb8();
        let small = image::imageops::resize(&img, 640, 640 * out.height as u32 / out.width as u32, image::imageops::FilterType::Triangle);
        let path = format!("tmp/region_preview/thumb_{group:08X}.png");
        small.save(&path).unwrap();
        println!("{group:08X}: {}×{} {}ms -> {path}", out.width, out.height, t0.elapsed().as_millis());
    }
}
