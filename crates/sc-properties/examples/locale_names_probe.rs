//! Export English names: <Game.package> <output.json> <locale.package> [...]
//! Inspect content: some installations swap locale directory contents.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert!(args.len() >= 3, "<Game.package> <output.json> <locale.package> [...]");
    let game = dbpf::Package::open(&args[0]).unwrap();
    let registry = sc_properties::region_3d::parse_template_registry(&game);
    let ids: std::collections::HashSet<u32> = registry.values()
        .flat_map(|(region, cities)| std::iter::once(*region).chain(cities.values().copied())).collect();
    let mut out = std::collections::BTreeMap::new();
    for path in &args[2..] {
        let package = dbpf::Package::open(path).unwrap();
        for (id, name) in sc_properties::region_3d::parse_locale_names(&package) {
            if ids.contains(&id) && !name.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c)) {
                out.insert(format!("{id:08X}"), name);
            }
        }
    }
    assert_eq!(out.len(), ids.len(), "Incomplete English name table");
    std::fs::write(&args[1], serde_json::to_string_pretty(&out).unwrap()).unwrap();
    println!("Exported {} English names", out.len());
}
