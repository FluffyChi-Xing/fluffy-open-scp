use super::*;
use dbpf::{write_uncompressed_overlay, OverlayEntry, Package, ResourceId};
use sc_properties::PropertyFile;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static TEST_SEQ: AtomicU64 = AtomicU64::new(0);
use sc_properties::{Kind, PropType, Property, PropertyEncoding, Value};

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "openscp-m1-{}-{}",
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    root.canonicalize().unwrap()
}

fn source_package(root: &Path) -> PathBuf {
    let property = |hash, value| {
        PropertyFile {
            claimed_count: 1,
            values: vec![Property {
                hash,
                prop_type: PropType::Int32,
                kind: Kind::Scalar(Value::Int32(value)),
                encoding: PropertyEncoding::default(),
            }],
        }
        .encode_canonical()
        .unwrap()
    };
    let entries = [
        OverlayEntry::new(
            ResourceId {
                type_id: 0x00B1B104,
                group: 0x40E0C000,
                instance: 0x8D007745,
            },
            property(0x0A8A41DA, 100),
        ),
        OverlayEntry::new(
            ResourceId {
                type_id: 0x00B1B104,
                group: 0x40E1C000,
                instance: 0x8D007745,
            },
            property(0x0BBBCDED, 80),
        ),
    ];
    let path = root.join("source.package");
    fs::write(&path, write_uncompressed_overlay(&entries).unwrap()).unwrap();
    path
}

#[test]
fn clones_source_and_writes_property_overlay_with_readback() {
    let root = temp_root();
    source_package(&root);
    fs::write(
        root.join("asset.lot.json"),
        r#"{"units":[{"kind":"pathPoint"}],"editor":{"workbench":{"paths":[]}}}"#,
    )
    .unwrap();
    let schema = serde_json::json!({
        "$schema": "openscp.asset", "schema_version": 2,
        "id": "willis", "kind": "building-asset",
        "reference": {
            "package": "source.package",
            "unit": "00B1B104:40E0C000:8D007745",
            "lot": "00B1B104:40E1C000:8D007745",
            "unitTemplate": "00B1B104:48E0C000:4E01E311"
        },
        "scene": {"document": "asset.lot.json"},
        "semantics": {"properties": [
            {"tgi": "00B1B104:40E0C000:8D007745", "hash": "0x0A8A41DA", "value": 140000, "expected": 100},
            {"tgi": "00B1B104:40E1C000:8D007745", "hash": "0x0BBBCDED", "value": 219, "expected": 80}
        ]},
        "outputs": {"package": "willis.package"}
    });
    let result = build_from_schema(&root, &schema.to_string()).unwrap();
    assert_eq!(result.resource_count, 2);
    assert_eq!(result.patched_resources, 2);
    let output = Package::open(&result.path).unwrap();
    let unit = output
        .entry(ResourceId {
            type_id: 0x00B1B104,
            group: 0x40E0C000,
            instance: 0x8D007745,
        })
        .unwrap();
    let file = PropertyFile::parse(&output.read(unit).unwrap()).unwrap();
    assert_eq!(file.values[0].scalar(), Some(&Value::Int32(140000)));
    assert!(result.path.starts_with(root.join(".build")));
    drop(output);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn refuses_shared_template_patch_by_default() {
    let root = temp_root();
    source_package(&root);
    let schema = serde_json::json!({
        "$schema": "openscp.asset", "schema_version": 2,
        "id": "willis", "kind": "building-asset",
        "reference": {
            "package": "source.package",
            "unitTemplate": "00B1B104:40E0C000:4E01E311"
        },
        "scene": {"document": "asset.lot.json"},
        "semantics": {"properties": [{
            "tgi": "00B1B104:40E0C000:4E01E311", "hash": "0x1234", "value": 1
        }]},
        "outputs": {"package": "willis.package"}
    });
    let error = build_from_schema(&root, &schema.to_string()).unwrap_err();
    assert!(error.contains("shared template"));
    let _ = fs::remove_dir_all(root);
}

fn schema() -> serde_json::Value {
    serde_json::json!({
        "$schema": "openscp.asset", "schema_version": 2, "id": "willis", "kind": "building-asset",
        "reference": {"package": "source.package", "unit": "00B1B104:40E0C000:8D007745"},
        "scene": {"document": "asset.lot.json"},
        "semantics": {"properties": [{"tgi": "00B1B104:40E0C000:8D007745", "hash": "0A8A41DA", "value": 140000, "expected": 100}]},
        "outputs": {"package": "willis.package"}
    })
}

#[test]
fn repeat_build_preserves_source_opaque_resources_and_artifacts() {
    let root = temp_root();
    let source = source_package(&root);
    let mut resources = connector::read_source(&source).unwrap();
    let opaque = ResourceId {
        type_id: 0x2F4E681B,
        group: 0,
        instance: 123,
    };
    resources.insert(opaque, vec![0, 255, 1, 7, 9]);
    fs::write(&source, connector::link(&resources).unwrap()).unwrap();
    let original = fs::read(&source).unwrap();
    let first = build_from_schema(&root, &schema().to_string()).unwrap();
    let second = build_from_schema(&root, &schema().to_string()).unwrap();
    assert_eq!(first.build_hash, second.build_hash);
    assert_eq!(first.sha256, second.sha256);
    assert_eq!(original, fs::read(&source).unwrap());
    assert_eq!(
        connector::read_source(&first.path).unwrap()[&opaque],
        resources[&opaque]
    );
    let build = root.join(".build").join(first.build_hash);
    assert!(build.join("plan/im.json").is_file());
    assert!(build
        .join("artifacts/property/00B1B104-40E0C000-8D007745.property")
        .is_file());
    assert!(build.join("reports/m1-build.json").is_file());
    assert!(build.join("artifacts/scene/asset.lot.json").is_file());
    assert!(build.join("artifacts/resource-manifest.json").is_file());
    assert!(!root.join("build").exists());
    assert_eq!(fs::read_dir(root.join(".build")).unwrap().count(), 1);
    resources.insert(opaque, vec![42]);
    fs::write(&source, connector::link(&resources).unwrap()).unwrap();
    let third = build_from_schema(&root, &schema().to_string()).unwrap();
    assert_ne!(first.source_hash, third.source_hash);
    assert_ne!(second.build_hash, third.build_hash);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_patch_creates_no_build_and_does_not_change_source() {
    let root = temp_root();
    let source = source_package(&root);
    let bytes = fs::read(&source).unwrap();
    let mut raw = schema();
    raw["semantics"]["properties"][0]["expected"] = serde_json::json!(999);
    assert!(build_from_schema(&root, &raw.to_string())
        .unwrap_err()
        .contains("expected"));
    assert_eq!(bytes, fs::read(source).unwrap());
    assert!(!root.join(".build").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unsupported_writers_and_malformed_patches_fail_in_pure_plan() {
    let mut raw = schema();
    raw["scene"]["model"] = serde_json::json!("model.lotm");
    assert!(plan::plan_from_schema(&raw.to_string()).is_err());
    raw["scene"].as_object_mut().unwrap().remove("model");
    raw["semantics"]["properties"][0]["tgi"] = serde_json::json!("2F4E681B:40E0C000:8D007745");
    assert!(plan::plan_from_schema(&raw.to_string()).is_err());
    raw = schema();
    raw["semantics"]["allowSharedTemplate"] = serde_json::json!("false");
    assert!(plan::plan_from_schema(&raw.to_string()).is_err());
    raw = schema();
    raw["semantics"]["properties"][0]["propertyTypo"] = serde_json::json!("int32");
    assert!(plan::plan_from_schema(&raw.to_string()).is_err());
}

#[test]
fn guards_omitted_landmark_template_and_source_parent_targets() {
    let mut raw = schema();
    raw["semantics"]["properties"][0]["tgi"] = serde_json::json!("00B1B104:40E1C000:4E01E311");
    assert!(plan::plan_from_schema(&raw.to_string())
        .err()
        .unwrap()
        .contains("shared template"));
    raw["semantics"]["allowSharedTemplate"] = serde_json::json!(true);
    assert!(plan::plan_from_schema(&raw.to_string()).is_ok());
    let root = temp_root();
    let source = source_package(&root);
    let mut resources = connector::read_source(&source).unwrap();
    let unit = ResourceId {
        type_id: 0x00B1B104,
        group: 0x40E0C000,
        instance: 0x8D007745,
    };
    let parent = ResourceId {
        instance: 999,
        ..unit
    };
    resources.insert(parent, resources[&unit].clone());
    let patch = patch::parse_patch(serde_json::from_value(serde_json::json!({
        "tgi": "00B1B104:40E0C000:8D007745", "hash": "00B2CCCB", "propertyType": "key", "value": "00B1B104:40E0C000:000003E7"
    })).unwrap()).unwrap();
    resources.insert(
        unit,
        processor::process_property(&resources[&unit], &[patch]).unwrap(),
    );
    fs::write(source, connector::link(&resources).unwrap()).unwrap();
    raw = schema();
    raw["semantics"]["properties"][0]["tgi"] = serde_json::json!("00B1B104:40E0C000:000003E7");
    assert!(build_from_schema(&root, &raw.to_string())
        .unwrap_err()
        .contains("shared template"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn rejects_traversal_and_preserves_existing_build_on_corruption() {
    let root = temp_root();
    source_package(&root);
    let mut raw = schema();
    raw["reference"]["package"] = serde_json::json!("../source.package");
    assert!(build_from_schema(&root, &raw.to_string()).is_err());
    let result = build_from_schema(&root, &schema().to_string()).unwrap();
    fs::write(&result.path, b"do not overwrite").unwrap();
    assert!(build_from_schema(&root, &schema().to_string()).is_err());
    assert_eq!(fs::read(result.path).unwrap(), b"do not overwrite");
    assert_eq!(fs::read_dir(root.join(".build")).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn deferred_processor_does_not_parse_until_materialized() {
    let artifact = processor::defer_property(
        ResourceId {
            type_id: 0x00B1B104,
            group: 0,
            instance: 1,
        },
        vec![1],
        vec![],
    );
    assert!((artifact.materialize)().is_err());
}

#[test]
fn schema_hash_includes_patches_and_template_policy() {
    let raw = schema();
    let first = plan::plan_from_schema(&raw.to_string()).unwrap();
    let mut changed = raw.clone();
    changed["semantics"]["properties"][0]["value"] = serde_json::json!(42);
    assert_ne!(
        first.intermediate.schema_hash,
        plan::plan_from_schema(&changed.to_string())
            .unwrap()
            .intermediate
            .schema_hash
    );
    changed = raw;
    changed["semantics"]["allowSharedTemplate"] = serde_json::json!(true);
    assert_ne!(
        first.intermediate.schema_hash,
        plan::plan_from_schema(&changed.to_string())
            .unwrap()
            .intermediate
            .schema_hash
    );
}

#[test]
fn malformed_property_with_trailing_data_is_rejected_before_write() {
    let mut bytes = PropertyFile::default().encode_canonical().unwrap();
    bytes.extend_from_slice(&[1, 2, 3]);
    assert!(processor::process_property(&bytes, &[]).is_err());
}

#[test]
fn rejects_scene_path_without_path_entry_before_staging() {
    let root = temp_root();
    source_package(&root);
    fs::write(
        root.join("asset.lot.json"),
        r#"{"editor":{"workbench":{"paths":[{"id":"path:1","pointIds":[]}]}}}"#,
    )
    .unwrap();
    let error = build_from_schema(&root, &schema().to_string()).unwrap_err();
    assert!(error.contains("missing path entry"));
    assert!(!root.join(".build").exists());
    fs::remove_dir_all(root).unwrap();
}

/// Opt-in fixture: clones the entire supplied package to a disposable project.
/// SC_M1_PACKAGE must contain the Willis Unit at 00B1B104:40E0C000:8D007745.
#[test]
#[ignore = "requires a local game package with the Willis Unit"]
fn real_willis_property_package_readback() {
    let source = std::env::var("SC_M1_PACKAGE").expect("set SC_M1_PACKAGE");
    let root = temp_root();
    fs::copy(&source, root.join("source.package")).unwrap();
    let source_resources = connector::read_source(&root.join("source.package")).unwrap();
    let unit = ResourceId {
        type_id: 0x00B1B104,
        group: 0x40E0C000,
        instance: 0x8D007745,
    };
    let original = PropertyFile::parse(&source_resources[&unit]).unwrap();
    let property = original.get(0x0A8A41DA).expect("Willis cost");
    let (cost, next, expected_value) = match property.scalar().unwrap() {
        Value::Int32(v) => (
            serde_json::json!(v),
            serde_json::json!(v + 1),
            Value::Int32(v + 1),
        ),
        Value::UInt32(v) => (
            serde_json::json!(v),
            serde_json::json!(v + 1),
            Value::UInt32(v + 1),
        ),
        Value::Float(v) => (
            serde_json::json!(v),
            serde_json::json!(v + 1.0),
            Value::Float(v + 1.0),
        ),
        other => panic!("cost is not numeric: {other:?}"),
    };
    let mut raw = schema();
    raw["semantics"]["properties"][0]["expected"] = cost.clone();
    raw["semantics"]["properties"][0]["value"] = next.clone();
    raw["semantics"]["properties"][0]["propertyType"] =
        serde_json::json!(property.prop_type.name());
    let result = build_from_schema(&root, &raw.to_string()).unwrap();
    let output = connector::read_source(&result.path).unwrap();
    for (id, bytes) in &source_resources {
        if *id != unit {
            assert_eq!(&output[id], bytes, "unchanged {id}");
        }
    }
    let updated = PropertyFile::parse(&output[&unit]).unwrap();
    assert_eq!(
        updated.get(0x0A8A41DA).unwrap().scalar(),
        Some(&expected_value)
    );
    for property in original.values.iter().filter(|p| p.hash != 0x0A8A41DA) {
        assert_eq!(updated.get(property.hash).unwrap().kind, property.kind);
    }
    assert_eq!(
        source_resources,
        connector::read_source(&root.join("source.package")).unwrap()
    );
    println!(
        "Willis readback: {} resources; cost {} -> {}; output {}",
        result.resource_count, cost, next, result.sha256
    );
    fs::remove_dir_all(root).unwrap();
}

/// Opt-in fixture: uses the real DLC0 package as a read-only reference and
/// applies one guarded LotSize patch for each workbench sample.
#[test]
#[ignore = "requires the local asset-build fixture package"]
fn real_asset_sample_property_fixture_readback() {
    let fixture = std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT");
    for sample in ["university", "commercial"] {
        let root = PathBuf::from(&fixture).join(sample);
        let raw = fs::read_to_string(root.join("schema.json")).unwrap();
        let result = build_from_schema(&root, &raw).unwrap_or_else(|error| panic!("{sample}: {error}"));
        assert_eq!(result.patched_resources, 1);
        assert!(result.path.is_file());
        let report = fs::read(
            root.join(".build")
                .join(&result.build_hash)
                .join("reports/m1-build.json"),
        )
        .unwrap();
        assert!(serde_json::from_slice::<serde_json::Value>(&report).is_ok());
    }
}

#[test]
#[ignore = "requires local asset-build fixtures"]
fn real_commercial_property_fixture_builds_for_manual_package_export() {
    let fixture = PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let root = fixture.join("commercial");
    let raw = fs::read_to_string(root.join("schema.json")).unwrap();
    let result = build_from_schema(&root, &raw).unwrap();
    assert!(result.path.is_file());
}

#[test]
#[ignore = "requires local DLC0 and Graphics packages"]
fn real_university_path_entry_fixture_builds_and_reads_back() {
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let graphics_path = PathBuf::from(
        std::env::var("SC_ASSET_GRAPHICS_PACKAGE").expect("set SC_ASSET_GRAPHICS_PACKAGE"),
    );
    let root = fixture.join("university");
    let source_path = root.join("SimCity_DLC0.package");
    let original_source = fs::read(&source_path).unwrap();
    let scene = r#"{"version":1,"editor":{"workbench":{"paths":[{"id":"path:1","entry":"675b61b1","pointIds":[],"source":{"typeId":11645188,"group":1374732288,"instance":2639719421}}]}}}"#;
    fs::write(root.join("asset.lot.json"), scene).unwrap();
    let mut resources = connector::read_source(&source_path).unwrap();
    let graphics = Package::open(&graphics_path).unwrap();
    let path_entry = ResourceId {
        type_id: 0x00B1_B104,
        group: 0x40E1_C400,
        instance: 0x675B_61B1,
    };
    let entry = graphics
        .entry(path_entry)
        .expect("university PathEntry in Graphics package");
    resources.insert(path_entry, graphics.read(entry).unwrap());
    fs::write(&source_path, connector::link(&resources).unwrap()).unwrap();
    let raw = fs::read_to_string(root.join("schema.json")).unwrap();
    let result = build_from_schema(&root, &raw).unwrap();
    let output = connector::read_source(&result.path).unwrap();
    assert!(output.contains_key(&path_entry));
    let build_root = result.path.parent().unwrap().parent().unwrap();
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(build_root.join("artifacts/resource-manifest.json")).unwrap(),
    )
    .unwrap();
    assert!(manifest["path_entry"].as_array().unwrap().iter().any(|value| {
        value.as_str() == Some("00B1B104-40E1C400-675B61B1")
    }));
    assert!(build_root
        .join("artifacts/path_entry/00B1B104-40E1C400-675B61B1.resource")
        .is_file());
    assert!(result.path.is_file());
    fs::write(&source_path, original_source).unwrap();
    let _ = fs::remove_file(root.join("asset.lot.json"));
}

#[test]
#[ignore = "requires AppData commercial sample and local fixture package"]
fn real_commercial_sample_rejects_unclosed_path_before_staging() {
    let mods_root = PathBuf::from(std::env::var("OPEN_SCP_MODS_ROOT").expect("set OPEN_SCP_MODS_ROOT"));
    let fixture = PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let root = fixture.join("commercial");
    let _ = fs::remove_dir_all(root.join(".build"));
    fs::copy(mods_root.join("商业地标资产开发/asset.lot.json"), root.join("asset.lot.json")).unwrap();
    let raw = fs::read_to_string(root.join("schema.json")).unwrap();
    let error = build_from_schema(&root, &raw).unwrap_err();
    assert!(error.contains("path semantic validation failed"));
    assert!(!root.join(".build").exists());
}
