//! Read-only geometry/unit fixture for reproducing native decal projection.
//! native_decal_fixture <Graphics.package> <lot hex> <output.json>
use dbpf::Package;
use sc_properties::{LotEditorDocument, PropertyFile, assemble_units};
use serde_json::json;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pkg = Package::open(&args[1]).unwrap();
    let lot = u32::from_str_radix(&args[2], 16).unwrap();
    let entry = pkg
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x00b1b104 && e.id.instance == lot)
        .unwrap();
    let raw = pkg.read(entry).unwrap();
    let properties = PropertyFile::parse(&raw).unwrap();
    let document = LotEditorDocument::from_property_file(properties.clone());
    let model = document.model.unwrap().instance;
    let model_entry = pkg
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x2f4e681b && e.id.instance == model)
        .unwrap();
    let bytes = pkg.read(model_entry).unwrap();
    let file = rw4::Rw4File::parse(&bytes).unwrap();
    let mut meshes = Vec::new();
    for section in file.sections_of_type(rw4::SectionType::MESH) {
        let mesh = file.decode_mesh(&bytes, section.number).unwrap();
        let positions: Vec<_> = mesh
            .vertices
            .iter()
            .map(|v| v.position().unwrap())
            .collect();
        meshes.push(json!({"positions": positions, "triangles": mesh.triangles}));
    }
    let units = assemble_units(&properties);
    std::fs::write(
        &args[3],
        serde_json::to_vec(&json!({
            "lot":lot, "model":model, "meshes":meshes, "units":units.units,
        }))
        .unwrap(),
    )
    .unwrap();
    println!("fixture {}: lot {lot:08X}, model {model:08X}", args[3]);
}
