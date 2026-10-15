//! Inspect named map assets and optionally export the native shoreline foam.
//! Usage: cargo run -p sc-properties --example map_layer_assets -- <game-data> [output-dir]
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let base = std::path::Path::new(args.first().expect("<game-data> [output-dir]"));
    if let Ok(package) = dbpf::Package::open(base.join("SimCity_RegionTerrain0.package")) {
        for group in [0xC04182E4, 0xD01FA985, 0xC2A9C48F] {
            let params = sc_properties::region_3d::region_water_params(&package, group);
            println!("{group:08X} water: {params:?}");
            if let Some(out) = args.get(1) {
                std::fs::create_dir_all(out).unwrap();
                std::fs::write(std::path::Path::new(out).join(format!("water-{group:08X}.json")),
                    serde_json::to_vec_pretty(&params).unwrap()).unwrap();
            }
        }
    }
    let names = ["waterHeight", "waterNormals", "waterChoppy", "beach_foam", "Impostor_Forest"];
    for name in ["SimCity_App.package", "SimCity_Graphics.package", "SimCity_Game.package", "SimCityDataEP1.package"] {
        let Ok(package) = dbpf::Package::open(base.join(name)) else { continue };
        for entry in package.entries() {
            // InitImpostorClass reads these four model keys from 5F804D7E.
            // Inspect every matching TGI: instance IDs alone do not identify a resource.
            let forest_model = [0x4DF43690, 0xC2FBD178, 0x1113B131, 0x89D658DF].contains(&entry.id.instance);
            let named_asset = names.iter().find(|n| sc_properties::region_3d::fnv1_lower(n) == entry.id.instance);
            if named_asset.is_none() && !forest_model { continue; }
            let asset = named_asset.copied().unwrap_or("forest-model-reference");
            let bytes = package.read(entry).unwrap();
            println!("{name}: {asset} {:08X}:{:08X}:{:08X} {} bytes", entry.id.type_id, entry.id.group, entry.id.instance, bytes.len());
            if entry.id.type_id == 0x00B1B104 {
                println!("{}", sc_properties::PropertyFile::parse(&bytes).unwrap());
            }
            if forest_model && entry.id.type_id == 0x2F4E681B {
                let file = rw4::Rw4File::parse(&bytes).unwrap();
                for section in file.sections_of_type(rw4::SectionType::MESH) {
                    match file.decode_mesh(&bytes, section.number) {
                        Ok(mesh) => println!("  mesh #{}: {} vertices", section.number, mesh.vertices.len()),
                        Err(error) => println!("  mesh #{}: {error}", section.number),
                    }
                }
                for section in file.sections_of_type(rw4::SectionType::TEXTURE) {
                    let texture = file.decode_texture(&bytes, section.number).unwrap();
                    println!("  embedded texture #{}: {}x{}", section.number, texture.width, texture.height);
                }
                println!("  material sections: {}", file.sections_of_type(rw4::SectionType::MATERIAL).count());
                for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
                    println!("  material: {:?}", file.decode_material(&bytes, section.number).unwrap());
                }
            }
            if asset == "beach_foam" && args.len() > 1 {
                let file = rw4::Rw4File::parse(&bytes).unwrap();
                let section = file.sections_of_type(rw4::SectionType::TEXTURE).next().unwrap().number;
                let texture = file.decode_texture(&bytes, section).unwrap();
                let image = image::RgbaImage::from_raw(texture.width.into(), texture.height.into(), texture.decode_top_mip_rgba().unwrap()).unwrap();
                let out = std::path::Path::new(&args[1]);
                std::fs::create_dir_all(out).unwrap();
                image.save(out.join("beach-foam.png")).unwrap();
                println!("  exported {}x{}", texture.width, texture.height);
            }
        }
    }
}
