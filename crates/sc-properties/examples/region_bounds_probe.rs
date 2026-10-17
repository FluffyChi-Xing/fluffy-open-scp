//! Verify region-visible-area crops against every shipped RT0 region and plot.
fn main() {
    let root = "D:/ea-games/SimCity/SimCityData";
    let p = dbpf::Package::open(format!("{root}/SimCity_RegionTerrain0.package")).unwrap();
    let ep1 = dbpf::Package::open(format!("{root}/SimCityDataEP1.package")).unwrap();
    for region in sc_properties::region_map::list_regions(&p) {
        let r =
            sc_properties::region_3d::region_3d(&p, region.group, Some(&ep1), None, None).unwrap();
        let half = r.size as f32 * r.meters_per_pixel / 2.0;
        assert_eq!(r.origin_world, (-half, -half));
        let height = image::load_from_memory(&r.height_png).unwrap();
        assert_eq!((height.width(), height.height()), (r.size, r.size));
        let mut outside = 0;
        for plot in &r.plots {
            let radius = if plot.kind == "city" { 1024.0 } else { 768.0 };
            if plot.x.abs() + radius > half || plot.y.abs() + radius > half {
                outside += 1;
                println!(
                    "  boundary overlap: {} ({},{}) radius={}",
                    plot.uid, plot.x, plot.y, radius
                );
            }
        }
        println!(
            "{:08X}: side={}m, {} plots, outside={outside}",
            region.group,
            half * 2.0,
            r.plots.len()
        );
        assert_eq!(
            outside, 0,
            "visible area excluded a plot in {:08X}",
            region.group
        );
    }
}
