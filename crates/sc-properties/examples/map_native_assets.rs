//! Export private local fixtures; game assets and saves must not be committed.
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let base = std::path::Path::new(&args[0]);
    let graphics = dbpf::Package::open(base.join("SimCity_Graphics.package")).unwrap();
    let game = dbpf::Package::open(base.join("SimCity_Game.package")).unwrap();
    let app = dbpf::Package::open(base.join("SimCity_App.package")).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    let plots: Vec<_> = fixture["plots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| sc_properties::region_3d::Region3DPlot {
            x: p["x"].as_f64().unwrap() as f32,
            y: p["y"].as_f64().unwrap() as f32,
            z: p["z"].as_f64().unwrap() as f32,
            uid: p["uid"].as_str().unwrap().into(),
            kind: p["kind"].as_str().unwrap().into(),
            name: None,
            name_en: None,
        })
        .collect();
    let (reference, layers) = if args.get(4).is_some_and(|a| a == "--discover") {
        sc_properties::region_state::discover_save(std::path::Path::new(&args[1]),0xC04182E4,&plots).expect("matching region reference")
    } else {
        (std::path::PathBuf::from(&args[1]),sc_properties::region_state::load_save(std::path::Path::new(&args[1]), 0xC04182E4, &plots).unwrap())
    };
    println!("Reference: {}",reference.display());
    let ids: std::collections::BTreeSet<_> = layers
        .sources
        .iter()
        .flat_map(|s| s.curves.iter().map(|c| c.entry_id))
        .collect();
    let mut roads = sc_properties::map_assets::road_assets(
        &game,
        &[&graphics, &app],
        &ids.into_iter().collect::<Vec<_>>(),
    );
    sc_properties::map_assets::append_road_instances(
        &mut roads,
        &game,
        &[&graphics, &app],
        &layers.regional_units,
    );
    let trees = sc_properties::map_assets::forest_models(&graphics);
    println!(
        "{} sources; {} trees; {} road definitions; {} textures; {} unsupported components",
        layers.sources.len(),
        trees.len(),
        roads.ribbons.len(),
        roads.textures.len(),
        roads.unsupported_components.len()
    );
    std::fs::write(
        &args[3],
        serde_json::to_vec(
            &serde_json::json!({"forestModels":trees,"saveLayers":layers,"roadAssets":roads,"referenceDirectory":reference}),
        )
        .unwrap(),
    )
    .unwrap();
}
