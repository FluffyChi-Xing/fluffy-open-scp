//! Read-only audit of regional unit definitions and bridge models.
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let units: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let ids: std::collections::BTreeSet<_> = units
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["id"].as_u64().unwrap() as u32)
        .collect();
    for name in [
        "SimCity_Game.package",
        "SimCity_Graphics.package",
        "SimCity_App.package",
    ] {
        let pkg = dbpf::Package::open(std::path::Path::new(&args[0]).join(name)).unwrap();
        for e in pkg.entries() {
            if ids.contains(&e.id.instance) && e.id.type_id == 0x00B1B104 {
                let pf = sc_properties::PropertyFile::parse(&pkg.read(e).unwrap()).unwrap();
                println!("UNIT {name} {:08X}:{:08X}\n{pf}", e.id.group, e.id.instance);
            }
            if [0xE2C6A2EB, 0xCC152350, 0x30360939].contains(&e.id.instance)
                && e.id.type_id == 0x2F4E681B
            {
                let bytes = pkg.read(e).unwrap();
                let f = rw4::Rw4File::parse(&bytes).unwrap();
                println!("MODEL {name} {:08X}:{:08X}", e.id.group, e.id.instance);
                for s in f.sections_of_type(rw4::SectionType::MESH) {
                    let m = f.decode_mesh(&bytes, s.number).unwrap();
                    println!(
                        "mesh {} verts {} uv {:?}",
                        s.number,
                        m.vertices.len(),
                        m.vertices.first().and_then(|v| v.uv())
                    );
                }
                println!(
                    "textures {} export {:?}",
                    f.sections_of_type(rw4::SectionType::TEXTURE).count(),
                    sc_properties::map_assets::model(&pkg, e.id.instance)
                        .map(|m| (m.positions.len(), m.diffuse_png_base64.len()))
                );
            }
        }
    }
}
