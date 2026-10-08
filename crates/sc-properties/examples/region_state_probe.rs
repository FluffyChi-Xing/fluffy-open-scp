//! Decode an already gunzipped retail state; outputs JSON to an explicit path.
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let data = std::fs::read(&args[0]).unwrap();
    let state = sc_properties::region_state::read_state(&data).unwrap();
    println!("v{} environment {:08X}: {} maps, {} curves", state.version, state.environment, state.maps.len(), state.curves.len());
    for name in ["coal", "ore", "oil", "waterTable", "forest", "soil", "wind", "coalMap", "oreMap", "oilMap", "waterTableMap"] {
        let id = sc_properties::region_3d::fnv1_lower(name);
        println!("{name}: {id:08X} found={}", state.maps.iter().any(|m| m.id == id));
    }
    std::fs::write(&args[1], serde_json::to_vec(&state).unwrap()).unwrap();
}
