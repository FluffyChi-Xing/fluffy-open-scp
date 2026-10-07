//! Export real region fields for the terrain-color regression/visual harness.
//! cargo run -p sc-properties --example terrain_color_probe -- <game-data> <group-hex> <output>
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3, "<game-data> <group-hex> <output>");
    let base = std::path::Path::new(&args[0]);
    let group = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).unwrap();
    let out = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    let app = dbpf::Package::open(base.join("SimCity_App.package")).unwrap();
    for (name, png) in sc_properties::region_3d::terrain_detail_pngs(&app) {
        std::fs::write(out.join(format!("{name}.png")), png).unwrap();
    }
    let terrain = dbpf::Package::open(base.join("SimCity_RegionTerrain0.package")).unwrap();
    let ep1 = dbpf::Package::open(base.join("SimCityDataEP1.package")).ok();
    let game = dbpf::Package::open(base.join("SimCity_Game.package")).ok();
    let locale = dbpf::Package::open(base.join("Locale/zh-tw/Data.package")).ok();
    let (b0, b1, b2) = sc_properties::region_3d::assembled_ed(&terrain, group).unwrap();
    let h = sc_properties::region_3d::assembled_height(&terrain, group).unwrap();
    let mut sums = [0.0; 3];
    let mut land = 0;
    let mut grass_sum = 0.0;
    for y in 0..2048 {
        for x in 0..2048 {
            if h[y * 2 * 4096 + x * 2] <= 5028 { continue; }
            let k = y * 2048 + x;
            for (sum, v) in sums.iter_mut().zip([b0[k], b1[k], b2[k]]) { *sum += f64::from(v); }
            grass_sum += (f64::from(b0[k]) * f64::from(b2[k])).sqrt() / 255.0;
            land += 1;
        }
    }
    println!("{group:08X} land={land}, mean B/G/R={:?}, sqrt(soil*water)={:.3}",
        sums.map(|v| v / land as f64), grass_sum / land as f64);
    let mut region = sc_properties::region_3d::region_3d(&terrain, group, ep1.as_ref(), game.as_ref(), locale.as_ref()).unwrap();
    println!("{:?} / {:?}, desert={}", region.display_name, region.display_name_en, region.desert);
    std::fs::write(out.join("height.png"), &region.height_png).unwrap();
    std::fs::write(out.join("ground.png"), &region.ground_png).unwrap();
    region.height_png.clear();
    region.ground_png.clear();
    std::fs::write(out.join("region.json"), serde_json::to_vec_pretty(&region).unwrap()).unwrap();
}
