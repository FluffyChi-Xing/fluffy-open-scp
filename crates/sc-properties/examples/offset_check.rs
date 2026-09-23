//! 探针：验证地块/画刷 → PNG 像素换算是否在界内（复刻前端 toPx）。仅开发用。
fn main() {
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    let out = sc_properties::region_map::render_region_png(&p, 0xBEAF_0510, None).unwrap();
    println!("PNG {}x{} origin=({:.0},{:.0})", out.width, out.height, out.origin_world.0, out.origin_world.1);
    println!("plots {}:", out.plots.len());
    for (wx, wy) in &out.plots {
        let px = (wx - out.origin_world.0) / out.meters_per_pixel;
        let py = (wy - out.origin_world.1) / out.meters_per_pixel;
        let inside = px >= 0.0 && py >= 0.0 && px <= out.width as f32 && py <= out.height as f32;
        println!("  world ({wx:.0},{wy:.0}) -> px ({px:.0},{py:.0}) inside={inside}");
    }
    let total: usize = out.brushes.iter().map(|(_, s)| s.len()).sum();
    println!("brush stamps: {total}");
    for (name, stamps) in &out.brushes {
        for (wx, wy) in stamps {
            let px = (wx - out.origin_world.0) / out.meters_per_pixel;
            let py = (wy - out.origin_world.1) / out.meters_per_pixel;
            let inside = px >= 0.0 && py >= 0.0 && px <= out.width as f32 && py <= out.height as f32;
            println!("  {name} ({wx:.0},{wy:.0}) -> ({px:.0},{py:.0}) inside={inside}");
        }
    }
}
