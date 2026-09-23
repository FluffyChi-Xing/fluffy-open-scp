//! 探针：用正式 render_region_png 管线（SHARED_ED_GRID + 数据驱动水位 + 资源图层）渲染验证。仅开发用。
fn main() {
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package")
        .unwrap();
    std::fs::create_dir_all("tmp/region_preview").unwrap();
    for group in [0xD01F_A985u32, 0xB12D_E348, 0xBEAF_0510] {
        let t0 = std::time::Instant::now();
        let out = sc_properties::region_map::render_region_png(&p, group, None).unwrap();
        let path = format!("tmp/region_preview/pipeline_{group:08X}.png");
        std::fs::write(&path, &out.png).unwrap();
        for (kind, png) in &out.resource_layers {
            std::fs::write(
                format!("tmp/region_preview/res_{group:08X}_{kind}.png"),
                png,
            )
            .unwrap();
        }
        println!(
            "{group:08X}: {}×{} 水位={} 荒漠={} 地块={} 画刷={} 图层={} {}ms -> {path}",
            out.width,
            out.height,
            out.water_plane,
            out.desert,
            out.plots.len(),
            out.brushes.len(),
            out.resource_layers.len(),
            t0.elapsed().as_millis()
        );
    }
}
