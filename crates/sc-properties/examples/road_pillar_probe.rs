use dbpf::Package;
fn main() {
    let root =
        std::path::Path::new("D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData");
    let g = Package::open(root.join("SimCity_Game.package")).unwrap();
    let p = Package::open(root.join("SimCity_Graphics.package")).unwrap();
    let a = Package::open(root.join("SimCity_App.package")).unwrap();
    let assets = sc_properties::map_assets::road_appearance(
        &g,
        0x09558B7E,
        std::env::args()
            .nth(1)
            .map(|s| u32::from_str_radix(&s, 16).unwrap())
            .unwrap_or(0xDC6C325F),
        &[&p, &a],
    )
    .unwrap();
    std::fs::create_dir_all("tmp/road-review").unwrap();
    std::fs::write(
        "tmp/road-review/road.json",
        serde_json::to_vec(&assets).unwrap(),
    )
    .unwrap();
    let mut configs = std::collections::BTreeSet::new();
    for e in g.entries().iter().filter(|e| e.id.type_id == 0x00B1B104) {
        if let Ok(d) = g.read(e) {
            if let Ok(f) = sc_properties::PropertyFile::parse(&d) {
                if f.get(240591623).is_some_and(|p| {
                    matches!(
                        &p.kind,
                        sc_properties::Kind::Scalar(sc_properties::Value::Bool(true))
                    )
                }) {
                    configs.insert(e.id.instance);
                }
            }
        }
    }
    for e in g
        .entries()
        .iter()
        .filter(|e| e.id.type_id == 0x00B1B104 && e.id.group == 0x09558B7E)
    {
        let Ok(data) = g.read(e) else { continue };
        let Ok(f) = sc_properties::PropertyFile::parse(&data) else {
            continue;
        };
        let keys = sc_properties::actor_graphics::keys(&f, 0xBCC53802);
        if keys.iter().any(|k| configs.contains(&k.instance_id)) {
            println!("SAME_SUPPORT {:08X}", e.id.instance);
        }
    }
    for (id, s) in &assets.sweeps {
        println!("appearance {id:08X}");
        for v in s {
            println!(
                "model {:08X} offset {:?} scale {:?} rot {:?} instance={} stack={}",
                v.model, v.offset, v.scale, v.rotation, v.instance, v.stack_to_ground
            );
        }
    }
}
