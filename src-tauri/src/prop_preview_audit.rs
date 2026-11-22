//! Read-only reproduction against the installed original DLC assets.
use super::*;

#[test]
#[ignore = "requires SC_ASSET_ROOT pointing to the original SimCityData directory"]
fn airship_preview_audit() {
    let root = std::path::PathBuf::from(std::env::var("SC_ASSET_ROOT").unwrap());
    let manager = PackageManager::new();
    for name in [
        "SimCity_App.package",
        "SimCity_Game.package",
        "SimCity_Graphics.package",
        "SimCity_DLC0.package",
        "SimCityDataEP1.package",
    ] {
        manager
            .insert(Package::open(root.join(name)).unwrap())
            .unwrap();
    }
    let resolved = crate::prop_models::resolve_all(
        &manager,
        &[0x6BD73FAC, 0x836A0617, 0x259B7086, 0xE5D80819],
    )
    .unwrap();
    println!("RESOLVED {resolved:#?}");
    for prop in &resolved {
        let Some(model) = prop.models.first() else {
            continue;
        };
        let pkg = manager
            .all_packages_with_ids()
            .unwrap()
            .into_iter()
            .find(|(id, _)| Some(*id) == prop.package_id)
            .unwrap()
            .1;
        let entry = pkg
            .entries()
            .iter()
            .find(|e| e.id.instance == model.instance && e.id.type_id == RW4_MODEL_TYPE)
            .unwrap();
        let data = pkg.read(entry).unwrap();
        let file = rw4::Rw4File::parse(&data).unwrap();
        for sec in file.sections_of_type(rw4::SectionType::MESH) {
            match file.decode_mesh(&data, sec.number) {
                Ok(mesh) => println!(
                    "MODEL {:08X} mesh {} export={} uvKind={}",
                    model.instance,
                    sec.number,
                    mesh.is_exportable(),
                    mesh_uv_kind(&mesh)
                ),
                Err(e) => println!("MESH ERROR {e}"),
            }
        }
        for sec in file.sections_of_type(rw4::SectionType::MATERIAL) {
            let m = resolve_material_resources(&file, &data, &pkg, &manager, sec.number);
            if prop.resource_id == 0x6BD73FAC {
                assert!(m.slot0_png.is_some());
                assert!(m.params_f32.is_none());
                for mesh_sec in file.sections_of_type(rw4::SectionType::MESH) {
                    let mesh = file.decode_mesh(&data, mesh_sec.number).unwrap();
                    assert_eq!(prop_preview_uv_kind(&mesh, m.params_f32.is_some()), 1);
                }
            }
            println!(
                "MATERIAL {} params={} slot0={} base={} tint={} palette={} diag={}",
                sec.number,
                m.params_f32.is_some(),
                m.slot0_png.is_some(),
                m.base_color_png.is_some(),
                m.tint_png.is_some(),
                m.palette_png.is_some(),
                m.diag
            );
        }
    }
    assert_eq!(
        resolved.len(),
        4,
        "all four original prop prototypes must resolve"
    );
}
