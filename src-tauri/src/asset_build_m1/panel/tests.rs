use super::*;
use crate::asset_build_m1::{
    connector, package::BuildResources, plan::plan_from_schema, property_edit::encode,
    stages::BuildContext,
};
use dbpf::{OverlayEntry, ResourceId, write_uncompressed_overlay};
use sc_properties::{Kind, PropertyFile, Value};
use serde_json::json;
use std::{fs, path::PathBuf};
fn root() -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tmp/panel-tests")
        .join(format!(
            "openscp-panel-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir_all(&p).unwrap();
    p.canonicalize().unwrap()
}
fn base() -> crate::asset_build_m1::plan::PropertyBuildPlan {
    plan_from_schema(&json!({"$schema":"openscp.asset","schema_version":2,"id":"panel","kind":"building-asset","reference":{"package":"source.package"},"scene":{"document":"asset.lot.json","authoredGround":true},"semantics":{"menu":{"title":"Panel"}},"outputs":{"package":"000_panel.package","mode":"full"}}).to_string()).unwrap()
}
#[test]
fn opt_in_output_retains_runtime_and_layout_without_changing_other_resources() {
    let root = root();
    let layout = ResourceId {
        type_id: 0x67771F5C,
        group: 0x0B07405A,
        instance: 0x123,
    };
    let children = [
        164191353u32,
        212238278,
        252147891,
        215183945,
        215191523,
        226403367,
        248008263,
    ]
    .map(|control| json!({"controlID":control}));
    let shell = json!({"type":"cLayout","children":children});
    let script = components()
        .iter()
        .map(|c| format!("simcity.RolloverItems.{};", c.native))
        .collect::<String>();
    let bytes = write_uncompressed_overlay(&[
        OverlayEntry::new(layout, serde_json::to_vec(&shell).unwrap()),
        OverlayEntry::new(sources::GAME_JS, script.into_bytes()),
    ])
    .unwrap();
    fs::write(root.join("ui.package"), &bytes).unwrap();
    let schema = json!({"$schema":"openscp.panel","schema_version":1,"shell":layout.to_string(),"hover":[],"click":["maintenance"],"power":false,"missingFeedback":"hide"});
    fs::write(root.join("panel.json"), schema.to_string()).unwrap();
    let dependency = spec::Dependency {
        schema: "panel.json".into(),
        sources: vec![sources::source(&root.join("ui.package")).unwrap()],
        runtime: manifest::RUNTIME_VERSION.into(),
    };
    fs::write(
        root.join("package.json"),
        json!({"format":"openscp.mod","dependencies":{"panel":dependency,"menu":{"title":"keep"}}})
            .to_string(),
    )
    .unwrap();
    let mut plan = base();
    attach(&root, &mut plan).unwrap();
    let before = serde_json::to_value(&plan).unwrap();
    assert!(before.get("panel").is_some());
    let mut ctx = BuildContext::new(root.clone(), plan);
    ctx.menu_instance = Some(0xABC);
    let asset = ResourceId {
        type_id: 0x00B1B104,
        group: 0x40E1C000,
        instance: 0xABC,
    };
    let unit = ResourceId {
        group: 0x40E0C000,
        ..asset
    };
    let empty = PropertyFile::parse(&0u32.to_be_bytes()).unwrap();
    let original = encode(&empty).unwrap();
    let mut pool = connector::Resources::new();
    pool.insert(asset, original.clone());
    pool.insert(unit, original.clone());
    ctx.resources = BuildResources::from_source(pool);
    let detail = apply(&mut ctx).unwrap();
    assert_eq!(detail["runtime"]["version"], manifest::RUNTIME_VERSION);
    assert_eq!(ctx.resources.get(&unit), Some(&original));
    assert_eq!(fs::read(root.join("ui.package")).unwrap(), bytes);
    let output = PropertyFile::parse(ctx.resources.get(&asset).unwrap()).unwrap();
    assert_eq!(
        output.get(0x0CAB_B2FC).unwrap().kind,
        Kind::Array(vec![Value::UInt32(hash("OpenSCPPanel_empty"))])
    );
    ctx.resources.retain_identity_closure(0xABC);
    assert!(!ctx.resources.contains_key(&sources::GAME_JS));
    assert_eq!(
        ctx.resources
            .keys()
            .filter(|id| id.type_id == 0x67771F5C)
            .count(),
        1
    );
    assert!(ctx.panel_runtime.is_some());
    let mut staging =
        crate::asset_build_m1::filesystem::Staging::create(&root, "panel-test").unwrap();
    publish_runtime(&mut staging, ctx.panel_runtime.as_ref().unwrap()).unwrap();
    assert!(staging.path.join(RUNTIME_OUTPUT).is_file());
    drop(staging);
    let current = manifest::read(&root).unwrap();
    let request = json!({"schema":schema,"dependency":dependency,"manifestRevision":current["manifestRevision"],"schemaRevision":current["schemaRevision"]});
    let saved = manifest::save(&root, &request).unwrap();
    let path = saved["schemaPath"].as_str().unwrap();
    assert!(path.starts_with("schemas/panel/"));
    assert!(root.join(path).is_file());
    let stored: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("package.json")).unwrap()).unwrap();
    assert_eq!(stored["dependencies"]["menu"]["title"], "keep");
    assert!(stored.pointer("/editor/workbench/panels").is_none());
    assert_eq!(
        manifest::save(&root, &request).unwrap_err(),
        "PANEL_REVISION_CONFLICT"
    );
    let inherited=manifest::save(&root,&json!({"schema":null,"manifestRevision":saved["manifestRevision"],"schemaRevision":saved["schemaRevision"]})).unwrap();
    assert!(inherited["schema"].is_null());
    assert!(root.join(path).is_file());
    fs::write(
        root.join("package.json"),
        r#"{"format":"openscp.mod","dependencies":{}}"#,
    )
    .unwrap();
    let mut inherited = base();
    let expected = serde_json::to_vec(&inherited).unwrap();
    attach(&root, &mut inherited).unwrap();
    assert!(inherited.panel.is_none());
    assert_eq!(serde_json::to_vec(&inherited).unwrap(), expected);
    fs::write(root.join("package.json"),json!({"format":"openscp.mod","dependencies":{"panel":dependency}}).to_string()).unwrap();
    fs::write(root.join("ui.package"),b"changed source").unwrap();
    assert!(attach(&root,&mut base()).unwrap_err().contains("PANEL_SOURCE_CHANGED"));
    assert!(
        root.starts_with(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("tmp/panel-tests")
                .canonicalize()
                .unwrap()
        )
    );
    fs::remove_dir_all(root).unwrap();
}
