//! Opt-in real-asset fixture export for browser profiling. No game data is committed.
use super::*;

#[test]
#[ignore = "requires SC_GAME_DATA and writes fixtures under tmp"]
fn export_property_render_fixture() {
    let base = PathBuf::from(std::env::var("SC_GAME_DATA").expect("SC_GAME_DATA"));
    let instance = u32::from_str_radix(&std::env::var("SC_LOT_INSTANCE").unwrap_or("1A7490A1".into()), 16).unwrap();
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tmp/property-render").join(format!("{instance:08X}"));
    std::fs::create_dir_all(&out).unwrap();
    let manager = PackageManager::new();
    let mut target = None;
    let mut model_package_id = None;
    let model_package = std::env::var("SC_MODEL_PACKAGE").unwrap_or("SimCity_Game.package".into());
    for name in ["SimCityDataEP1.package", "SimCity_Game.package", "SimCity_App.package", "SimCity_Graphics.package", "SimCity_DLC0.package"] {
        // Reproduce base-game reports without silently selecting an EP1 override.
        if name == "SimCityDataEP1.package" && std::env::var_os("SC_FIXTURE_BASE_GAME_ONLY").is_some() {
            continue;
        }
        let (id, package) = manager.insert(Package::open(base.join(name)).unwrap()).unwrap();
        if name == model_package {
            model_package_id = Some(id);
        }
        if target.is_none() {
            if let Some(entry) = package.entries().iter().find(|e| e.id.type_id == PROPERTY_RESOURCE_TYPE && e.id.instance == instance) {
                target = Some((id, Arc::clone(&package), entry.id));
            }
        }
    }
    let (id, package, key) = target.expect("lot property");
    let store = sc_store::Store::open(out.join("fixture.sqlite")).unwrap();
    let data = package.read(package.entry(key).unwrap()).unwrap();
    let mut session = build_lot_editor_session(&data, &package, &manager, &store,
        LotEditorSessionRequest { package_id: id, tgi: key.into() }, None).unwrap();
    if let Ok(model_hex) = std::env::var("SC_MODEL_INSTANCE") {
        let model_instance = u32::from_str_radix(model_hex.trim_start_matches("0x"), 16).unwrap();
        session.model_lods = vec![Some(LodModelRef {
            package_id: model_package_id.expect("requested model package must be loaded"),
            tgi: TgiDto { type_id: RW4_MODEL_TYPE, group: 0, instance: model_instance },
        })];
    }
    std::fs::write(out.join("session.json"), serde_json::to_vec(&session).unwrap()).unwrap();
    std::fs::write(out.join("properties.json"), serde_json::to_vec_pretty(&session.document.properties).unwrap()).unwrap();
    if let Some(mask) = session.document.lot_mask {
        for (_, pkg) in manager.all_packages_with_ids().unwrap() {
            if let Some(entry) = pkg.entries().iter().find(|e| e.id.type_id == RASTER_IMAGE_TYPE && e.id.instance == mask.instance) {
                let bytes = pkg.read(entry).unwrap();
                let raster = rw4::RasterImage::parse(&bytes).unwrap();
                eprintln!("mask {:08X} {}x{} mip bytes {:?}", mask.instance, raster.width, raster.height, raster.mips.iter().map(Vec::len).collect::<Vec<_>>());
                std::fs::write(out.join("mask.raster"), bytes).unwrap();
                break;
            }
        }
    }
    let export = |package_id: u64, key: &TgiDto, filename: &str| {
        let package = manager.get(package_id).unwrap();
        let data = package.read(package.entry(key.clone().into()).unwrap()).unwrap();
        let file = rw4::Rw4File::parse(&data).unwrap();
        let payload = build_lot_model_payload_for_test(&file, &data, &package, &manager, key.instance);
        std::fs::write(out.join(filename), payload).unwrap();
    };
    if let Some(model) = session.model_lods.iter().flatten().next() {
        export(model.package_id, &model.tgi, "model.lotm");
    }
    let mut ids: Vec<_> = session.units.iter().filter_map(|unit| match unit {
        sc_properties::LotUnit::Prop { resource_id, .. } => *resource_id,
        _ => None,
    }).collect();
    ids.sort_unstable();
    ids.dedup();
    let props = crate::prop_models::resolve_all(&manager, &ids).unwrap();
    for prop in &props {
        if let Some(package_id) = prop.package_id {
            for (index, key) in prop.models.iter().enumerate() {
                export(package_id, key, &format!("prop-{:08X}-{index}.lotm", prop.resource_id));
            }
        }
    }
    std::fs::write(out.join("props.json"), serde_json::to_vec(&props).unwrap()).unwrap();
    let mut simf = Vec::new();
    simf.extend_from_slice(&SIM_PARTS_MAGIC.to_le_bytes());
    simf.extend_from_slice(&1u32.to_le_bytes());
    simf.extend_from_slice(&0u32.to_le_bytes());
    let mut count = 0u32;
    for (kind, keys) in [(0u8, &SIM_BODY_KEYS[..]), (1u8, &SIM_HEAD_KEYS[..])] {
        for key in keys {
            for (_, pkg) in manager.all_packages_with_ids().unwrap() {
                let Some(entry) = pkg.entries().iter().find(|e| e.id.type_id == RW4_MODEL_TYPE && e.id.instance == *key) else { continue };
                let data = pkg.read(entry).unwrap();
                let file = rw4::Rw4File::parse(&data).unwrap();
                let lotm = build_lot_model_payload_for_test(&file, &data, &pkg, &manager, *key);
                simf.extend_from_slice(&key.to_le_bytes());
                simf.push(kind);
                simf.extend_from_slice(&(lotm.len() as u32).to_le_bytes());
                simf.extend_from_slice(&lotm);
                count += 1;
                break;
            }
        }
    }
    simf[8..12].copy_from_slice(&count.to_le_bytes());
    std::fs::write(out.join("sims.simf"), simf).unwrap();
    println!("Fixture exported to {}", out.display());
}
