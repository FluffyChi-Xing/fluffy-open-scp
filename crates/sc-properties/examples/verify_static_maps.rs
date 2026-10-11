//! Opt-in validation against an installed game's packages; no game data is committed.
//! cargo run -p sc-properties --example verify_static_maps -- <SimCityData> [output-dir]
use base64::Engine;
use sc_properties::{map_assets, region_3d, region_bootstrap};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("expected SimCityData directory")?,
    );
    let output = std::env::args().nth(2).map(std::path::PathBuf::from);
    if let Some(path) = &output {
        std::fs::create_dir_all(path)?;
    }
    let ep1 = dbpf::Package::open(root.join("SimCityDataEP1.package"))?;
    let game = dbpf::Package::open(root.join("SimCity_Game.package"))?;
    let app = dbpf::Package::open(root.join("SimCity_App.package"))?;
    let locale = dbpf::Package::open(root.join("Locale/zh-tw/Data.package")).ok();
    let mut maps = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let encode = |bytes| base64::engine::general_purpose::STANDARD.encode(bytes);
    for file in [
        "SimCity_RegionTerrain0.package",
        "SimCity_RegionTerrain1.package",
    ] {
        let terrain = dbpf::Package::open(root.join(file))?;
        for region in
            region_3d::region_list_named(&terrain, Some(&ep1), Some(&game), locale.as_ref())
        {
            assert!(seen.insert(region.group));
            let layers = region_bootstrap::load(&terrain, &ep1, region.group)?;
            let ids = layers
                .sources
                .iter()
                .flat_map(|s| s.curves.iter().map(|c| c.entry_id))
                .collect::<std::collections::BTreeSet<_>>();
            let mut roads =
                map_assets::road_assets(&game, &[&app, &ep1], &ids.into_iter().collect::<Vec<_>>());
            map_assets::append_road_instances(
                &mut roads,
                &game,
                &[&app, &ep1],
                &layers.regional_units,
            );
            assert!(
                roads.unsupported_components.is_empty(),
                "unsupported roads: {:?}",
                roads.unsupported_components
            );
            let cities = layers.sources.iter().filter(|s| s.bounds.is_some()).count();
            assert_eq!(cities, region.plot_count);
            for source in layers.sources.iter().filter(|s| s.bounds.is_some()) {
                assert!(source.curves.is_empty());
                for id in [
                    0xD779C976, 0xD74AF22B, 0x76D10FF2, 0x0F92DC56, 0x1D52BE3E, 0x28F896A4,
                ] {
                    let m = source
                        .maps
                        .iter()
                        .find(|m| m.id == id)
                        .expect("missing resource field");
                    assert_eq!(m.values.len(), 128 * 128);
                }
            }
            let group = format!("{:08X}", region.group);
            println!("{group} {}: {cities} cities", layers.template);
            maps.push(serde_json::json!({"group":group,"template":layers.template,"cities":cities,"name":region.display_name,"nameEn":region.display_name_en}));
            if let Some(output) = &output {
                let data = region_3d::region_3d(
                    &terrain,
                    region.group,
                    Some(&ep1),
                    Some(&game),
                    locale.as_ref(),
                )?;
                let textures: std::collections::HashMap<_, _> =
                    region_3d::terrain_detail_pngs(&app)
                        .into_iter()
                        .map(|(k, v)| (k, encode(v)))
                        .collect();
                let dto = serde_json::json!({"heightPngBase64":encode(data.height_png),"groundPngBase64":encode(data.ground_png),"terrainTextures":textures,"staticLayers":layers,"staticStatus":"ready","roadAssets":roads,"size":data.size,"metersPerPixel":data.meters_per_pixel,"originWorld":data.origin_world,"waterZ":data.water_z,"heightDiv":data.height_div,"heightBias":data.height_bias,"desert":data.desert,"displayName":data.display_name,"displayNameEn":data.display_name_en,"plots":data.plots,"brushes":data.brushes});
                std::fs::write(
                    output.join(format!("{group}.json")),
                    serde_json::to_vec(&dto)?,
                )?;
            }
        }
    }
    assert_eq!(seen.len(), region_bootstrap::catalog(&ep1)?.len());
    println!(
        "Verified {} maps, {} cities",
        maps.len(),
        maps.iter()
            .map(|m| m["cities"].as_u64().unwrap())
            .sum::<u64>()
    );
    if let Some(output) = output {
        std::fs::write(
            output.join("catalog.json"),
            serde_json::to_vec_pretty(&maps)?,
        )?;
    }
    Ok(())
}
