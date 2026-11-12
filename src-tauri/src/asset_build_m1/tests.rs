use super::*;
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use sc_properties::PropertyFile;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static TEST_SEQ: AtomicU64 = AtomicU64::new(0);
use sc_properties::{Kind, PropType, Property, PropertyEncoding, Value};

pub(super) fn temp_root() -> PathBuf {
    // 沙箱环境常拒绝 $TMP 的 canonicalize;优先用工作区 target 目录(cargo
    // 本就写它),系统临时目录兜底。build_from_schema 自身会对 root 再
    // canonicalize,这里非规范路径也能跑。
    let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp");
    let candidates = [workspace_target, std::env::temp_dir()];
    for base in candidates {
        let root = base.join(format!(
            "openscp-m1-{}-{}",
            std::process::id(),
            TEST_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        if fs::create_dir_all(&root).is_ok() {
            return root.canonicalize().unwrap_or(root);
        }
    }
    panic!("cannot allocate test temp root");
}

/// 给夹具 lot 补主体三要件（2026-11-08 /37 参照 lot 主体契约：缺
/// BoundingBox/LotSize/LotMask 的夹具会被 load-inputs 以 DERIVED variant
/// 拒绝）。已有列不覆盖。
pub(crate) fn with_master_lot_columns(mut values: Vec<Property>) -> Vec<Property> {
    // 类型必须按 vanilla 语义给（BoundingBox/Vector2/Key），否则
    // menu 注册读遮罩时会报 "lot mask is not a Key"。
    let extra = [
        Property {
            hash: 0x00F9EFBA,
            prop_type: PropType::BoundingBox,
            kind: Kind::Scalar(Value::BoundingBox {
                min: [-32.0, -32.0, 0.0],
                max: [32.0, 32.0, 32.0],
            }),
            encoding: PropertyEncoding::default(),
        },
        Property {
            hash: 0x0CCB7FC8,
            prop_type: PropType::Vector2,
            kind: Kind::Scalar(Value::Vector2([64.0, 64.0])),
            encoding: PropertyEncoding::default(),
        },
        Property {
            hash: 0x0CCB7FD5,
            prop_type: PropType::Key,
            kind: Kind::Scalar(Value::Key(sc_properties::Key {
                type_id: 0x2F4E681C,
                group: 0,
                instance: 0x1111,
            })),
            encoding: PropertyEncoding::default(),
        },
    ];
    for property in extra {
        if values.iter().any(|existing| existing.hash == property.hash) {
            continue;
        }
        values.push(property);
    }
    values
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
        // lot 夹具必须是主体形态（2026-11-08 /37 参照 lot 主体契约）：
        // 0x0BBBCDED 数据列 + 主体三要件（BoundingBox/LotSize/LotMask），
        // 否则 load-inputs 会以 DERIVED variant 拒绝。
        OverlayEntry::new(
            ResourceId {
                type_id: 0x00B1B104,
                group: 0x40E1C000,
                instance: 0x8D007745,
            },
            PropertyFile {
                claimed_count: 4,
                values: vec![
                    Property {
                        hash: 0x0BBBCDED,
                        prop_type: PropType::Int32,
                        kind: Kind::Scalar(Value::Int32(80)),
                        encoding: PropertyEncoding::default(),
                    },
                    Property {
                        hash: 0x00F9EFBA,
                        prop_type: PropType::BoundingBox,
                        kind: Kind::Scalar(Value::BoundingBox {
                            min: [-32.0, -32.0, 0.0],
                            max: [32.0, 32.0, 32.0],
                        }),
                        encoding: PropertyEncoding::default(),
                    },
                    Property {
                        hash: 0x0CCB7FC8,
                        prop_type: PropType::Vector2,
                        kind: Kind::Scalar(Value::Vector2([64.0, 64.0])),
                        encoding: PropertyEncoding::default(),
                    },
                    Property {
                        hash: 0x0CCB7FD5,
                        prop_type: PropType::Key,
                        kind: Kind::Scalar(Value::Key(sc_properties::Key {
                            type_id: 0x2F4E681C,
                            group: 0,
                            instance: 0x1111,
                        })),
                        encoding: PropertyEncoding::default(),
                    },
                ],
            }
            .encode_canonical()
            .unwrap(),
        ),
        // The lot's LotMask must resolve to an actual raster in strict menu
        // tests; a key-only placeholder is intentionally rejected.
        OverlayEntry::new(
            ResourceId {
                type_id: 0x2F4E681C,
                group: 0,
                instance: 0x1111,
            },
            super::materials::encode_rgba_raster(1, 1, &[[255, 255, 255, 255]]).unwrap(),
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
        r#"{"units":[{"kind":"pathPoint","point":[0,0,0],"pointIndex":0}],"editor":{"workbench":{"paths":[]}}}"#,
    )
    .unwrap();
    let schema = serde_json::json!({
        "$schema": "openscp.asset", "schema_version": 2,
        "id": "willis", "kind": "building-asset",
        "reference": {
            "package": "source.package",
            "unit": "00B1B104:40E0C000:8D007745",
            "lot": "00B1B104:58F0C000:8D007745",
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
    assert_eq!(result.resource_count, 3);
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
    fs::write(root.join("asset.lot.json"), "{}").unwrap();
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
    let stale = root.join(".build").join("a".repeat(64));
    fs::create_dir_all(stale.join("packages")).unwrap();
    fs::write(stale.join("packages/old.package"), b"stale").unwrap();
    let second = build_from_schema(&root, &schema().to_string()).unwrap();
    assert_eq!(first.build_hash, second.build_hash);
    assert!(!stale.exists(), "a new build removes stale hash artifacts");
    assert_eq!(first.sha256, second.sha256);
    assert_eq!(original, fs::read(&source).unwrap());
    // Delta contract: the output carries only touched resources; the opaque
    // reference resource resolves from the base package at game load time.
    let output = connector::read_source(&first.path).unwrap();
    assert!(
        !output.contains_key(&opaque),
        "untouched opaque resource stays out"
    );
    let patched = ResourceId {
        type_id: 0x00B1B104,
        group: 0x40E0C000,
        instance: 0x8D007745,
    };
    assert!(output.contains_key(&patched), "patched property is written");
    let build = root.join(".build").join(first.build_hash);
    assert!(build.join("plan/im.json").is_file());
    assert!(
        build
            .join("artifacts/property/00B1B104-40E0C000-8D007745.property")
            .is_file()
    );
    assert!(build.join("reports/m1-build.json").is_file());
    assert!(build.join("artifacts/scene/asset.lot.json").is_file());
    assert!(build.join("artifacts/resource-manifest.json").is_file());
    assert!(!root.join("build").exists());
    // .build holds exactly one build directory plus the shared ledger/cache dirs.
    let build_entries: Vec<_> = fs::read_dir(root.join(".build"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        build_entries.len(),
        4,
        "build dir + ledger dir + cache dir + pool-last.json: {build_entries:?}"
    );
    assert!(build_entries.contains(&"ledger".to_string()));
    assert!(build_entries.contains(&"cache".to_string()));
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
    assert!(
        build_from_schema(&root, &raw.to_string())
            .unwrap_err()
            .contains("expected")
    );
    assert_eq!(bytes, fs::read(source).unwrap());
    assert!(!root.join(".build").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unsupported_writers_and_malformed_patches_fail_in_pure_plan() {
    // scene.model (RW4 lowering) plans fine since the model compiler landed;
    // non-Property patch targets and malformed patches still fail the plan.
    let mut raw = schema();
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
    assert!(
        plan::plan_from_schema(&raw.to_string())
            .err()
            .unwrap()
            .contains("shared template")
    );
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
    assert!(
        build_from_schema(&root, &raw.to_string())
            .unwrap_err()
            .contains("shared template")
    );
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
    // One build directory survived the corruption; the shared ledger/cache
    // dirs are its siblings (stages.rs writes .build/ledger, G3 writes
    // .build/cache).
    let build_entries: Vec<_> = fs::read_dir(root.join(".build"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        build_entries.len(),
        4,
        "build dir + ledger dir + cache dir + pool-last.json: {build_entries:?}"
    );
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
fn rejects_scene_path_without_type_before_staging() {
    let root = temp_root();
    source_package(&root);
    fs::write(
        root.join("asset.lot.json"),
        r#"{"editor":{"workbench":{"paths":[{"id":"path:1","pointIds":[]}]}}}"#,
    )
    .unwrap();
    let error = build_from_schema(&root, &schema().to_string()).unwrap_err();
    assert!(error.contains("path has no type"), "{error}");
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
        let result =
            build_from_schema(&root, &raw).unwrap_or_else(|error| panic!("{sample}: {error}"));
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
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
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
    assert!(
        manifest["path_entry"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| { value.as_str() == Some("00B1B104-40E1C400-675B61B1") })
    );
    assert!(
        build_root
            .join("artifacts/path_entry/00B1B104-40E1C400-675B61B1.resource")
            .is_file()
    );
    assert!(result.path.is_file());
    fs::write(&source_path, original_source).unwrap();
    let _ = fs::remove_file(root.join("asset.lot.json"));
}

#[test]
#[ignore = "requires AppData commercial sample and local fixture package"]
fn real_commercial_sample_rejects_unclosed_path_before_staging() {
    let mods_root =
        PathBuf::from(std::env::var("OPEN_SCP_MODS_ROOT").expect("set OPEN_SCP_MODS_ROOT"));
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let root = fixture.join("commercial");
    let _ = fs::remove_dir_all(root.join(".build"));
    fs::copy(
        mods_root.join("商业地标资产开发/asset.lot.json"),
        root.join("asset.lot.json"),
    )
    .unwrap();
    let raw = fs::read_to_string(root.join("schema.json")).unwrap();
    let error = build_from_schema(&root, &raw).unwrap_err();
    assert!(error.contains("path semantic validation failed"));
    assert!(!root.join(".build").exists());
}

#[test]
fn delta_output_writes_only_touched_resources() {
    let root = temp_root();
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
                instance: 0x11111111,
            },
            property(0x0A8A41DA, 100),
        ),
        OverlayEntry::new(
            ResourceId {
                type_id: 0x00B1B104,
                group: 0x40E1C000,
                instance: 0x11111111,
            },
            property(0x0A8A41DA, 200),
        ),
        OverlayEntry::new(
            ResourceId {
                type_id: 0x00B1B104,
                group: 0x11110000,
                instance: 0x33333333,
            },
            property(0x0AAAAAAA, 7),
        ),
    ];
    fs::write(
        root.join("source.package"),
        write_uncompressed_overlay(&entries).unwrap(),
    )
    .unwrap();
    fs::write(root.join("asset.lot.json"), "{}").unwrap();
    let schema = serde_json::json!({
        "$schema": "openscp.asset", "schema_version": 2,
        "id": "delta", "kind": "building-asset",
        "reference": { "package": "source.package", "unit": "00B1B104:40E0C000:11111111" },
        "scene": { "document": "asset.lot.json" },
        "semantics": { "properties": [
            {"tgi": "00B1B104:40E0C000:11111111", "hash": "0x0A8A41DA", "value": 140, "expected": 100}
        ]},
        "outputs": { "package": "delta.package" }
    });
    let result = build_from_schema(&root, &schema.to_string()).unwrap();
    assert!(result.delta_mode, "delta is the default output mode");
    assert_eq!(
        result.resource_count, 1,
        "only the patched property is written"
    );
    let output = Package::open(&result.path).unwrap();
    assert_eq!(output.entries().len(), 1);
    assert!(
        output
            .entry(ResourceId {
                type_id: 0x00B1B104,
                group: 0x11110000,
                instance: 0x33333333
            })
            .is_none(),
        "untouched reference resources stay out of the delta package"
    );
    drop(output);

    // Explicit full mode keeps the legacy reference-clone contract.
    let schema = serde_json::json!({
        "$schema": "openscp.asset", "schema_version": 2,
        "id": "delta", "kind": "building-asset",
        "reference": { "package": "source.package", "unit": "00B1B104:40E0C000:11111111" },
        "scene": { "document": "asset.lot.json" },
        "semantics": { "properties": [
            {"tgi": "00B1B104:40E0C000:11111111", "hash": "0x0A8A41DA", "value": 140, "expected": 100}
        ]},
        "outputs": { "package": "delta.package", "mode": "full" }
    });
    let result = build_from_schema(&root, &schema.to_string()).unwrap();
    assert!(!result.delta_mode);
    assert_eq!(
        result.resource_count, 3,
        "full mode re-serializes the reference clone"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn menu_registration_creates_new_identity_resources() {
    let root = temp_root();
    let lot = PropertyFile {
        claimed_count: 4,
        values: with_master_lot_columns(vec![Property {
            hash: 0x0BBBCDED,
            prop_type: PropType::Int32,
            kind: Kind::Scalar(Value::Int32(219)),
            encoding: PropertyEncoding::default(),
        }]),
    }
    .encode_canonical()
    .unwrap();
    let entries = [
        OverlayEntry::new(
            ResourceId {
                type_id: 0x00B1B104,
                group: 0x58F0C000,
                instance: 0x8D007745,
            },
            lot,
        ),
        // The lot-config declares a mask key; keep the fixture's dependency
        // closure honest with a minimal decodable 1x1 raster.
        OverlayEntry::new(
            ResourceId {
                type_id: 0x2F4E681C,
                group: 0,
                instance: 0x1111,
            },
            super::materials::encode_rgba_raster(1, 1, &[[255, 255, 255, 255]]).unwrap(),
        ),
    ];
    fs::write(
        root.join("source.package"),
        write_uncompressed_overlay(&entries).unwrap(),
    )
    .unwrap();
    fs::write(root.join("asset.lot.json"), "{}").unwrap();
    let schema = serde_json::json!({
        "$schema": "openscp.asset", "schema_version": 2,
        "id": "menu-test", "kind": "building-asset",
        "reference": { "package": "source.package", "lot": "00B1B104:58F0C000:8D007745" },
        "scene": { "document": "asset.lot.json" },
        "semantics": { "menu": {
            "title": "OpenSCP 测试公园",
            "description": "菜单录入机制测试",
            "menuParentInstance": "0xF970A822",
            "categoryInstance": "0x07049250",
            "unitTemplateInstance": "0x565B72B9",
            "lotTemplateInstance": "0x34CE7E14",
            "constructionCost": 1234
        }},
        "outputs": { "package": "menu-test.package" }
    });
    let result = build_from_schema(&root, &schema.to_string()).unwrap();
    let x = result.menu_instance.expect("menu instance allocated");
    assert!(result.delta_mode);
    // menu2 + unit leaf + lot leaf + locale + icon png + marquee jpg + lot mask；
    // 参照 lot 本身不被覆盖（新身份，不是 same-TGI 覆盖）。
    assert_eq!(result.resource_count, 7);
    let output = Package::open(&result.path).unwrap();
    assert_eq!(output.entries().len(), 7);
    assert!(
        output
            .entry(ResourceId {
                type_id: 0x00B1B104,
                group: 0x40E1C000,
                instance: 0x8D007745
            })
            .is_none(),
        "reference lot must stay untouched"
    );
    let read = |id: ResourceId| -> PropertyFile {
        PropertyFile::parse(&output.read(output.entry(id).unwrap()).unwrap()).unwrap()
    };
    let menu2 = read(ResourceId {
        type_id: 0x00B1B104,
        group: 0x09878A01,
        instance: x,
    });
    // 游戏加载器要求 0x8000 有效位（2026-10-30 实证：flags=0 整条不可见）
    for p in &menu2.values {
        assert!(
            p.encoding.flags & 0x8000 != 0,
            "hash {:08X} missing 0x8000",
            p.hash
        );
    }
    assert_eq!(
        menu2.get(0x0DB9FC63).map(|p| p.encoding.flags),
        Some(0x809c),
        "key array carries the canonical array flags"
    );
    assert_eq!(
        menu2.get(0x0A09F5FA).map(|p| p.encoding.flags),
        Some(0x801c),
        "text array carries the canonical text-array flags"
    );
    assert!(
        menu2.get(0x0C65D729).is_some(),
        "background icon reference present"
    );
    assert!(
        menu2.get(0x09756955).is_some(),
        "six-state icon block present"
    );
    assert!(
        matches!(menu2.get(0x0DB9FC63).map(|p| &p.kind),
            Some(Kind::Array(vs)) if matches!(&vs[0], Value::Key(k) if k.type_id == 0 && k.group == 0 && k.instance == 0x07049250)),
        "uiCategories targets the park container"
    );
    assert!(
        matches!(menu2.get(0x0975695F).map(|p| &p.kind),
            Some(Kind::Scalar(Value::Key(k))) if k.type_id == 0 && k.group == 0 && k.instance == x),
        "menuUnit points at the unit leaf with T/G=0 (NightHouse convention)"
    );
    assert!(
        matches!(menu2.get(0x00B2CCCB).map(|p| &p.kind),
            Some(Kind::Scalar(Value::Key(k))) if k.group == 0x48E1EB00 && k.instance == 0xF970A822),
        "menu parent is the park submenu root"
    );
    assert!(
        matches!(menu2.get(0x0A09F5FA).map(|p| &p.kind),
            Some(Kind::Array(vs)) if matches!(&vs[0], Value::Text(t) if t.table_id == x && t.instance_id == 1)),
        "title references the per-identity locale table, row 1 (19a1b3a)"
    );
    assert!(
        matches!(menu2.get(0x0A09F5FB).map(|p| &p.kind),
            Some(Kind::Array(vs)) if matches!(&vs[0], Value::Text(t) if t.table_id == x && t.instance_id == 2)),
        "description references the per-identity locale table, row 2"
    );
    let unit = read(ResourceId {
        type_id: 0x00B1B104,
        group: 0x40E0C000,
        instance: x,
    });
    assert!(
        matches!(unit.get(0x00B2CCCB).map(|p| &p.kind),
            Some(Kind::Scalar(Value::Key(k))) if k.group == 0x48E0C000 && k.instance == 0x565B72B9),
        "unit leaf inherits the configured cUnit template"
    );
    assert!(
        matches!(unit.get(0x08E17ED7).map(|p| &p.kind),
            Some(Kind::Scalar(Value::Key(k))) if k.group == 0x40E1C000 && k.instance == x),
        "unit leaf links its lot"
    );
    assert_eq!(
        unit.get(0x0A8A41DA).and_then(|p| p.scalar()),
        Some(&Value::Float(1234.0)),
        "construction cost override landed (Float32 — vanilla SCUnit type; Int32 renders as free)"
    );
    let lot_leaf = read(ResourceId {
        type_id: 0x00B1B104,
        group: 0x40E1C000,
        instance: x,
    });
    assert!(
        matches!(lot_leaf.get(0x00B2CCCB).map(|p| &p.kind),
            Some(Kind::Scalar(Value::Key(k))) if k.group == 0x40E1C000 && k.instance == 0x34CE7E14),
        "lot leaf inherits the configured lot template"
    );
    assert_eq!(
        lot_leaf.get(0x0BBBCDED).and_then(|p| p.scalar()),
        Some(&Value::Int32(219)),
        "cloned reference lot fields survive"
    );
    let locale = output
        .read(
            output
                .entry(ResourceId {
                    type_id: 0x0A98EAF0,
                    group: 0x02FABF01,
                    // locale 表随身份实例（per-identity，19a1b3a）。
                    instance: x,
                })
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        &locale[..3],
        &[0xEF, 0xBB, 0xBF],
        "locale table carries the BOM"
    );
    let parsed: serde_json::Value =
        serde_json::from_str(std::str::from_utf8(&locale[3..]).unwrap()).unwrap();
    // 作者约定:locale 表用固定行键(0x00000001=标题,0x00000002=简介),
    // 与菜单实例无关(build_donor_locale)。
    assert_eq!(
        parsed.get("0x00000001").and_then(|v| v.as_str()),
        Some("OpenSCP 测试公园")
    );
    assert_eq!(
        parsed.get("0x00000002").and_then(|v| v.as_str()),
        Some("菜单录入机制测试")
    );
    assert!(
        output
            .entry(ResourceId {
                type_id: 0x2F7D0004,
                group: 0x40E02400,
                instance: x
            })
            .is_some()
    );
    assert!(
        output
            .entry(ResourceId {
                type_id: 0x3F8662EA,
                group: 0,
                instance: x
            })
            .is_some()
    );
    drop(output);
    let _ = fs::remove_dir_all(root);
}

// ---- P0 黄金基线(2026-11-04,构建器拆分前的字节级行为锚) ----
//
// 写入场景文档 fixture(灯/prop/path 全家族)与 menu schema,供
// 黄金用例与确定性用例共用。
fn golden_fixture(root: &Path) -> String {
    // 自包含 donor 链源包(GENERIC_ASSET/INTERACTION_CHAIN/菜单链/donor
    // lot/PathEntry 675B61B1 + fixture donor lot 8D007745),由
    // examples/make_golden_fixture.rs 从真实参考包生成;模板链成员或
    // PathEntry 变更时重新生成即可。
    let source = include_bytes!("../../resources/sc5/golden-source.package");
    fs::write(root.join("source.package"), source).unwrap();
    // The historical golden package carried the generic cUnit template
    // 593B61D2 under the Asset-dictionary group.  The game resolves cUnit
    // templates from 48E0C000, so normalize the fixture at its boundary
    // instead of weakening the production closure validator.
    let source_path = root.join("source.package");
    let mut resources = connector::read_source(&source_path).unwrap();
    let legacy = ResourceId {
        type_id: 0x00B1B104,
        group: 0x40E1C000,
        instance: 0x593B61D2,
    };
    let canonical = ResourceId {
        type_id: 0x00B1B104,
        group: 0x48E0C000,
        instance: 0x593B61D2,
    };
    if let Some(bytes) = resources.get(&legacy).cloned() {
        resources.insert(canonical, bytes);
    }
    // The same historical package stores the donor lot only in the Asset
    // dictionary group.  Golden schemas intentionally exercise the typed
    // lot-config route, so provide the equivalent canonical donor TGI.
    let legacy_lot = ResourceId {
        type_id: 0x00B1B104,
        group: 0x40E1C000,
        instance: 0x8D007745,
    };
    let canonical_lot = ResourceId {
        type_id: 0x00B1B104,
        group: 0x58F0C000,
        instance: 0x8D007745,
    };
    if let Some(bytes) = resources.get(&legacy_lot).cloned() {
        resources.insert(canonical_lot, bytes);
    }
    fs::write(&source_path, connector::link(&resources).unwrap()).unwrap();
    fs::write(root.join("asset.lot.json"), r#"{
      "$schema": "openscp.lot-asset/1",
      "units": [
        {"id":"light:0","kind":"light","index":0,
         "transform":{"flags":15,"matrix":[1,0,0, 0,1,0, 0,0,1, 4,5,6]},
         "lightType":"Spot","color":[1.0,0.5,0.25],
         "outerRadius":12,"innerRadius":1.5,"diffuse":3,"specular":0.5,
         "length":20,"cullDistance":"Near","falloffStart":0.25,
         "isVolumetric":true,"volumetricStrength":2.5,"debugName":"Golden"},
        {"id":"prop:0","kind":"prop","index":0,"bin":9,
         "resourceId":305419896,"slot":0,"scale":1.5,
         "transform":{"flags":15,"matrix":[1,0,0, 0,1,0, 0,0,1, 2,0,4]}},
        {"id":"pathPoint:0","kind":"pathPoint","index":0,
         "point":[1,2,0],"tangent":[3,-4,0],"pointIndex":0},
        {"id":"pathPoint:1","kind":"pathPoint","index":1,
         "point":[5,6,0],"tangent":[7,-8,0],"pointIndex":1}
      ],
      "editor":{"workbench":{"paths":[
        {"id":"path:1","entry":"675b61b1","scope":"local","roadConnect":true,
         "pointIds":["pathPoint:0","pathPoint:1"],
         "source":{"packageId":1,"typeId":"0x00B1B104","group":"0x51F0C000","instance":"0x9D56EBFD"}}
      ]}}
    }"#).unwrap();
    serde_json::to_string(&serde_json::json!({
        "$schema": "openscp.asset", "schema_version": 2,
        "id": "golden", "kind": "building-asset",
        "reference": { "package": "source.package", "lot": "00B1B104:58F0C000:8D007745" },
        "scene": { "document": "asset.lot.json" },
        "semantics": { "menu": {
            "title": "OpenSCP 黄金用例",
            "description": "P0 全链路基线",
            "menuParentInstance": "0xF970A822",
            "categoryInstance": "0x07049250",
            "unitTemplateInstance": "0x565B72B9",
            "lotTemplateInstance": "0x34CE7E14",
            "constructionCost": 1234,
            "rotationDegrees": 0,
            "parentFree": true
        }},
        "outputs": { "package": "golden.package" }
    }))
    .unwrap()
}

fn column_first(file: &PropertyFile, hash: u32) -> Value {
    file.values
        .iter()
        .find(|p| p.hash == hash)
        .and_then(|p| match &p.kind {
            Kind::Array(values) => values.first().cloned(),
            Kind::Scalar(value) => Some(value.clone()),
            Kind::Empty => None,
        })
        .unwrap_or_else(|| panic!("missing column {hash:08X}"))
}

/// P0 黄金用例:menu(parent-free)模式下,场景文档全部 unit 家族
/// (灯/prop/path)在 Asset dict 的落点。四个历史雷区钉位:
/// 夜时段 key(0x1295_2639)、cull/体积光透传、transform flags 14、
/// path entry 组(40E1C400)。拆分/重构以本用例为行为基线。
#[test]
fn golden_full_chain_asset_dict_columns() {
    let root = temp_root();
    let schema = golden_fixture(&root);
    let result = build_from_schema(&root, &schema).unwrap();
    let x = result.menu_instance.expect("menu instance allocated");
    let output = Package::open(&result.path).unwrap();
    let asset = PropertyFile::parse(
        &output
            .read(
                output
                    .entry(ResourceId {
                        type_id: 0x00B1B104,
                        group: 0x40E1C000,
                        instance: x,
                    })
                    .unwrap(),
            )
            .unwrap(),
    )
    .unwrap();

    let key = |t: u32, g: u32, i: u32| {
        Value::Key(sc_properties::Key {
            type_id: t,
            group: g,
            instance: i,
        })
    };
    // -- 灯:14 列逐值(作者 packager 契约) --
    assert_eq!(column_first(&asset, 0x0CAA_8F10), key(0, 0, 0x8E43_F125));
    assert_eq!(column_first(&asset, 0x0CAA_8F11), key(0, 0, 0x2F0F_F9FD));
    match column_first(&asset, 0x0CAA_8F12) {
        Value::Transform(t) => {
            assert_eq!(t.flags, 14, "scale-less light transform uses flags 14");
            assert_eq!(t.unknown, None);
            assert_eq!(
                t.matrix,
                vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 4.0, 5.0, 6.0]
            );
        }
        other => panic!("light transform: {other:?}"),
    }
    assert_eq!(
        column_first(&asset, 0x0CAA_8F13),
        Value::ColorRgb {
            r: 1.0,
            g: 0.5,
            b: 0.25
        }
    );
    assert_eq!(column_first(&asset, 0x0CAA_8F14), Value::Float(12.0));
    assert_eq!(column_first(&asset, 0x0CAA_8F15), Value::Float(1.5));
    assert_eq!(column_first(&asset, 0x0CAA_8F16), Value::Float(3.0));
    assert_eq!(
        column_first(&asset, 0x0CAA_8F17),
        Value::String8("SC5_Golden".into())
    );
    assert_eq!(column_first(&asset, 0x0D4C_96B4), Value::Float(0.5));
    assert_eq!(column_first(&asset, 0x0D4C_AD23), Value::Float(20.0));
    assert_eq!(
        column_first(&asset, 0x0E01_200F),
        key(0, 0, 0x4394_551B),
        "authored Near cull tier"
    );
    assert_eq!(column_first(&asset, 0x0E4D_F244), Value::Float(0.25));
    assert_eq!(column_first(&asset, 0x0E98_D445), Value::Bool(true));
    assert_eq!(column_first(&asset, 0x0EC4_567E), Value::Float(2.5));
    // -- 夜时段绑定(历史 typo 雷区) --
    assert_eq!(column_first(&asset, 0x0CE7_1B10), key(0, 0, 0x8E43_F125));
    assert_eq!(
        column_first(&asset, 0x0CE7_1B11),
        key(0, 0, 0x1295_2639),
        "night period 311764537 — 0x1295_6239 was the dark-lights typo"
    );
    assert_eq!(
        column_first(&asset, 0x0CE7_1B12),
        Value::Vector2([0.875, 0.215_273_01])
    );
    assert_eq!(
        column_first(&asset, 0x0CE7_1B13),
        Value::Vector2([0.0, 0.0])
    );
    assert_eq!(
        column_first(&asset, 0x0F16_9C46),
        Value::Bool(true),
        "interior controller enabler"
    );
    // -- prop bin 9:资源键/带 scale 变换/索引 --
    assert_eq!(
        column_first(&asset, 0x0C12_EF20 + 9),
        key(0, 0, 0x1234_5678)
    );
    match column_first(&asset, 0x0C12_EF30 + 9) {
        Value::Transform(t) => {
            assert_eq!(t.flags, 15, "prop scale lives in the flags-15 slot");
            assert_eq!(t.unknown, Some(1.5));
        }
        other => panic!("prop transform: {other:?}"),
    }
    assert_eq!(column_first(&asset, 0x0C12_EF40 + 9), Value::Int32(0));
    // -- path:七列(entry 组/通用 scope/roadConnect 编码) --
    assert_eq!(
        column_first(&asset, 0x0CAA_680D),
        Value::Vector3([1.0, 2.0, 0.0])
    );
    assert_eq!(
        column_first(&asset, 0x0CB0_0ED8),
        Value::Vector3([3.0, -4.0, 0.0])
    );
    assert_eq!(column_first(&asset, 0x0CAA_6832), Value::Int32(0));
    let pairs: Vec<_> = asset
        .values
        .iter()
        .find(|p| p.hash == 0x0CAA_6841)
        .map(|p| match &p.kind {
            Kind::Array(v) => v.clone(),
            _ => unreachable!(),
        })
        .unwrap();
    assert_eq!(pairs, vec![Value::Int32(0), Value::Int32(1)]);
    assert_eq!(
        column_first(&asset, 0x0CC8_FE7A),
        key(0x00B1_B104, 0x40E1_C400, 0x675B_61B1)
    );
    assert_eq!(column_first(&asset, 0x0D9A_4E54), key(0, 0, 0xF44F_784A));
    // roadConnect:true = kRoadConnectFirst（s18 实机诊断修正；旧断言误钉
    // kRoadConnectNone=0xC186DA37）。
    assert_eq!(column_first(&asset, 0x0D9A_A2BB), key(0, 0, 0xF0F6_1CED));
    // -- 资源清单:menu 基线 6(menu2/unit/asset/locale/icon/marquee)
    //    + 旋转重编码后的 ground mask raster + donor 模型 RW4 + 材质 raster
    //    + PathEntry 675B61B1 闭包提升(993e7a3 起保留,s10 路径事故修复)。
    //    数量变化 = 行为变化,P1 拆分必须保持。 --
    assert!(
        output
            .entry(ResourceId {
                type_id: 0x00B1_B104,
                group: 0x40E1_C400,
                instance: 0x675B_61B1
            })
            .is_some(),
        "PathEntry 675B61B1 stays in the delta (993e7a3)"
    );
    assert_eq!(
        output.entries().len(),
        11,
        "menu baseline + graphics unit + mask raster + model RW4 + material raster + promoted PathEntry"
    );
    drop(output);
    let _ = fs::remove_dir_all(root);
}

/// P0 确定性:同一 schema 在两个独立根目录各构建一次 → 产物 byte 一致
/// (实例内容哈希/遍历顺序/容器写入全链确定性;P1 拆分的字节级不变量)。
#[test]
fn deterministic_output_two_fresh_builds_identical() {
    let mut outputs = Vec::new();
    for _ in 0..2 {
        let root = temp_root();
        let schema = golden_fixture(&root);
        let result = build_from_schema(&root, &schema).unwrap();
        outputs.push((result.menu_instance, fs::read(&result.path).unwrap()));
        let _ = fs::remove_dir_all(root);
    }
    assert_eq!(
        outputs[0].0, outputs[1].0,
        "menu instance derives from content"
    );
    assert_eq!(
        outputs[0].1, outputs[1].1,
        "fresh builds of identical inputs must be byte-identical"
    );
}

/// s10 无头构建 + 包内复核（手动，需本机项目与游戏包）：
/// cargo test -p fluffy-open-scp --release university_s10 -- --ignored --nocapture
/// 2026-11-05 大学项目迭代：10 树（bin12：14984C68×9 + C8B4254A×1）+
/// 9 车（bin0：0AB4DFE1）+ 11 灯。断言 PROP_ID 是脚本 resourceID（工作台
/// 直挂模型 id 已在场景侧治愈），bins/transforms/indices 长度按 vanilla
/// wire（ids 原型表×transforms 车位×indices 填充序 0..n）。
#[test]
#[ignore]
fn university_s10_headless_build_and_wire_check() {
    use sc_properties::lot_unit as uh;
    let root = Path::new(r"D:\rust\packages\fluffy-open-scp\tmp\s10-staging\university");
    let schema = fs::read_to_string(root.join("schemas/assets/m1.json")).expect("read m1.json");
    let result = build_from_schema(root, &schema).expect("build s10");
    println!(
        "output {} ({} resources, {} patched, menu {})",
        result.path.display(),
        result.resource_count,
        result.patched_resources,
        result
            .menu_instance
            .map(|m| format!("{m:08X}"))
            .unwrap_or_else(|| "-".into())
    );
    let package = Package::open(&result.path).expect("open output");
    let lot_tgi: Vec<(u32, u32)> = package
        .entries()
        .iter()
        .filter(|e| e.id.type_id == 0x00B1_B104)
        .map(|e| (e.id.group, e.id.instance))
        .collect();
    println!("B1B104 entries in output: {lot_tgi:02X?}");
    let lot = package
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x00B1_B104 && e.id.group == 0x40E1_C000)
        .expect("lot property in output");
    let data = package.read(lot).expect("read lot");
    let file = PropertyFile::parse_with_limits(&data, Default::default()).expect("parse lot");

    let key_instances = |hash: u32| -> Vec<u32> {
        file.values
            .iter()
            .filter(|p| p.hash == hash)
            .flat_map(|p| match &p.kind {
                Kind::Array(vs) => vs.iter().cloned().collect::<Vec<_>>(),
                Kind::Scalar(v) => vec![v.clone()],
                Kind::Empty => vec![],
            })
            .filter_map(|v| match v {
                Value::Key(k) => Some(k.instance),
                _ => None,
            })
            .collect()
    };
    let int_values = |hash: u32| -> Vec<i32> {
        file.values
            .iter()
            .filter(|p| p.hash == hash)
            .flat_map(|p| match &p.kind {
                Kind::Array(vs) => vs.iter().cloned().collect::<Vec<_>>(),
                Kind::Scalar(v) => vec![v.clone()],
                Kind::Empty => vec![],
            })
            .filter_map(|v| match v {
                Value::Int32(n) => Some(n),
                Value::UInt32(n) => Some(n as i32),
                _ => None,
            })
            .collect()
    };
    let count = |hash: u32| -> usize {
        file.values
            .iter()
            .filter(|p| p.hash == hash)
            .map(|p| match &p.kind {
                Kind::Array(vs) => vs.len(),
                Kind::Scalar(_) => 1,
                Kind::Empty => 0,
            })
            .sum()
    };

    // bin12 树：ids = [14984C68, C8B4254A]，transforms 10，indices 0..10
    let tree_ids = key_instances(uh::PROP_ID_BASE + 12);
    assert!(
        tree_ids.contains(&0x1498_4C68) && tree_ids.contains(&0xC8B4_254A),
        "bin12 树原型应含 14984C68+C8B4254A，实得 {tree_ids:08X?}"
    );
    assert_eq!(
        count(uh::PROP_TRANSFORM_BASE + 12),
        10,
        "bin12 车位/树位 transforms"
    );
    assert_eq!(
        int_values(uh::PROP_ID_BASE + 12 + 0x20),
        (0..10).collect::<Vec<_>>(),
        "bin12 slot 填充序 0..10"
    );
    // bin0 车：原版车辆资源族 E1/E2/E3 + 百分比填充标记，transforms 9
    let car_ids = key_instances(uh::PROP_ID_BASE);
    assert_eq!(
        car_ids,
        vec![0x0AB4_DFE1, 0x0AB4_DFE2, 0x0AB4_DFE3],
        "bin0 车原型族"
    );
    assert_eq!(count(uh::PROP_TRANSFORM_BASE), 9, "bin0 transforms");
    assert_eq!(
        int_values(uh::PROP_SLOT_BASE),
        (0..9).collect::<Vec<_>>(),
        "bin0 slot 填充序 0..9"
    );
    assert!(
        file.values
            .iter()
            .any(|p| p.hash == uh::PROP_PERCENT_FILL_BASE
                && matches!(p.kind, Kind::Scalar(Value::Bool(true)))),
        "bin0 车辆必须启用 mUseBinPercent"
    );
    // 灯源 11（本轮调整项）
    assert_eq!(count(uh::LIGHT_IDS), 11, "灯源 11 盏");
    // spawners：场景 9 个 spawner 经锚点库展开（s8 契约），只断言非空并打印
    let spawner_ids = key_instances(uh::SPAWNER_IDS);
    assert!(!spawner_ids.is_empty(), "spawner 锚点不能为空");
    assert_eq!(
        count(uh::SPAWNER_TRANSFORMS),
        count(uh::SPAWNER_IDS),
        "spawner ids/transforms 等长"
    );
    println!(
        "wire check OK: trees(bin12)={tree_ids:08X?} x10, cars(bin0)=[0AB4DFE1] x9, lights=11, spawner anchors={} ({:08X?})",
        spawner_ids.len(),
        spawner_ids
    );
}

// ---- behavior-bind（菜单/行为拆分）----

/// 合成 donor：Menu2/Unit/Lot/locale/图标共用一个身份 instance，locale 按
/// 插件作者约定携带标题/描述/哨兵三行。
fn behavior_donor_entries(instance: u32) -> Vec<(ResourceId, Vec<u8>)> {
    let locale_json = "{\r\n    \"0x00000001\" : \"原版大学\",\r\n    \"0x00000002\" : \"原始描述\",\r\n    \"0xffffffff\" : \"0xffffffff\"\r\n}";
    let mut locale = vec![0xEF, 0xBB, 0xBF];
    locale.extend_from_slice(locale_json.as_bytes());
    let menu2 = PropertyFile {
        claimed_count: 1,
        values: vec![Property {
            hash: 0x0A09F5FA,
            prop_type: PropType::Text,
            kind: Kind::Array(vec![Value::Text(sc_properties::Text {
                table_id: instance,
                instance_id: 1,
            })]),
            encoding: PropertyEncoding {
                flags: 0x801C,
                array_item_size: Some(sc_properties::TEXT_ARRAY_ITEM_SIZE),
            },
        }],
    }
    .encode_canonical()
    .unwrap();
    let key = |group: u32, instance: u32| {
        Value::Key(sc_properties::Key {
            type_id: 0,
            group,
            instance,
        })
    };
    let unit = PropertyFile {
        claimed_count: 3,
        values: vec![
            Property {
                hash: 0x00B2CCCB,
                prop_type: PropType::Key,
                kind: Kind::Scalar(key(0x48E0C000, 0x7E4B99E5)),
                encoding: PropertyEncoding::default(),
            },
            Property {
                hash: 0x0AFE27B1,
                prop_type: PropType::Key,
                kind: Kind::Scalar(key(0, 0x95A9D6FD)),
                encoding: PropertyEncoding::default(),
            },
            Property {
                hash: 0x0A0209A5,
                prop_type: PropType::Key,
                kind: Kind::Array(vec![key(0, 0xE7690450)]),
                encoding: PropertyEncoding {
                    flags: 0x809C,
                    array_item_size: Some(12),
                },
            },
        ],
    }
    .encode_canonical()
    .unwrap();
    let lot = PropertyFile {
        claimed_count: 4,
        values: with_master_lot_columns(vec![Property {
            hash: 0x0BBBCDED,
            prop_type: PropType::Int32,
            kind: Kind::Scalar(Value::Int32(219)),
            encoding: PropertyEncoding::default(),
        }]),
    }
    .encode_canonical()
    .unwrap();
    let at = |type_id: u32, group: u32, bytes: Vec<u8>| {
        (
            ResourceId {
                type_id,
                group,
                instance,
            },
            bytes,
        )
    };
    vec![
        at(0x00B1B104, 0x09878A01, menu2),
        at(0x00B1B104, 0x40E0C000, unit),
        at(0x00B1B104, 0x40E1C000, lot),
        // lot-config 形态（新身份路线的参照）：含主体三要件。
        at(
            0x00B1B104,
            0x58F0C000,
            PropertyFile {
                claimed_count: 4,
                values: with_master_lot_columns(vec![Property {
                    hash: 0x0BBBCDED,
                    prop_type: PropType::Int32,
                    kind: Kind::Scalar(Value::Int32(219)),
                    encoding: PropertyEncoding::default(),
                }]),
            }
            .encode_canonical()
            .unwrap(),
        ),
        at(0x0A98EAF0, 0x02FABF01, locale),
        // 冻结面：任一非属性资源（图标位）必须原样穿透。
        at(0x2F7D0004, 0x40E02400, vec![1, 2, 3, 4, 5]),
        (
            ResourceId {
                type_id: 0x2F4E681C,
                group: 0,
                instance: 0x1111,
            },
            super::materials::encode_rgba_raster(1, 1, &[[255, 255, 255, 255]]).unwrap(),
        ),
    ]
}

fn behavior_schema(mode: &str) -> serde_json::Value {
    serde_json::json!({
        "$schema": "openscp.asset", "schema_version": 2,
        "id": "behavior-test", "kind": "building-asset",
        "reference": { "package": "source.package", "lot": "00B1B104:58F0C000:8D007745" },
        "scene": { "document": "asset.lot.json" },
        "semantics": {
            "lotChange": true,
            "behavior": {
                "kind": "education-minimal",
                "titleOverride": "大学建筑测试包 s24",
                "studentCapacity": 250
            }
        },
        "outputs": { "package": "behavior-test.package", "mode": mode }
    })
}

fn behavior_donor(root: &Path) {
    let entries = behavior_donor_entries(0x8D007745)
        .into_iter()
        .map(|(id, bytes)| OverlayEntry::new(id, bytes))
        .collect::<Vec<_>>();
    fs::write(
        root.join("source.package"),
        write_uncompressed_overlay(&entries).unwrap(),
    )
    .unwrap();
    fs::write(root.join("asset.lot.json"), "{}").unwrap();
}

#[test]
fn behavior_full_clone_patches_unit_and_freezes_menu() {
    let root = temp_root();
    behavior_donor(&root);
    let schema = behavior_schema("full").to_string();
    let first = build_from_schema(&root, &schema).unwrap();

    // 1) 全克隆：输出 TGI 集与 donor 完全一致。
    let donor = connector::read_source(&root.join("source.package")).unwrap();
    let output_package = Package::open(&first.path).unwrap();
    let output_ids: Vec<ResourceId> = output_package
        .entries()
        .iter()
        .map(|entry| entry.id)
        .collect();
    let mut donor_ids: Vec<ResourceId> = donor.keys().copied().collect();
    donor_ids.sort_by_key(|id| (id.type_id, id.group, id.instance));
    let mut sorted_ids = output_ids.clone();
    sorted_ids.sort_by_key(|id| (id.type_id, id.group, id.instance));
    assert_eq!(
        sorted_ids, donor_ids,
        "full mode must carry every donor TGI and nothing else"
    );

    let read = |id: ResourceId| -> Vec<u8> {
        output_package
            .read(output_package.entry(id).unwrap())
            .unwrap()
    };
    let at = |type_id: u32, group: u32| ResourceId {
        type_id,
        group,
        instance: 0x8D007745,
    };

    // 2) 菜单冻结：Menu2 与图标字节不变。
    assert_eq!(
        read(at(0x00B1B104, 0x09878A01)),
        donor[&at(0x00B1B104, 0x09878A01)],
        "Menu2 must stay byte-identical"
    );
    assert_eq!(
        read(at(0x2F7D0004, 0x40E02400)),
        donor[&at(0x2F7D0004, 0x40E02400)],
        "icon bytes must stay identical"
    );

    // 3) locale 仅标题行变化。
    let locale_after = read(at(0x0A98EAF0, 0x02FABF01));
    let locale_text = String::from_utf8(locale_after).unwrap();
    assert!(
        locale_text.contains("\"大学建筑测试包 s24\""),
        "title overridden"
    );
    assert!(
        locale_text.contains("\"0x00000002\" : \"原始描述\""),
        "description row frozen"
    );
    assert!(
        locale_text.contains("\"0xffffffff\" : \"0xffffffff\""),
        "sentinel row frozen"
    );

    // 4) Unit 增量字段：parent/category/容量/教育 bundle 并入，树 bundle 保留。
    let unit = PropertyFile::parse(&read(at(0x00B1B104, 0x40E0C000))).unwrap();
    let key_instance = |file: &PropertyFile, hash: u32| match file.get(hash).map(|p| &p.kind) {
        Some(Kind::Scalar(Value::Key(k))) => Some((k.group, k.instance)),
        _ => None,
    };
    assert_eq!(
        key_instance(&unit, 0x00B2CCCB),
        Some((0x48E0C000, 0x7E4B99E5)),
        "unit parent stays on the education template"
    );
    assert_eq!(
        key_instance(&unit, 0x0AFE27B1),
        Some((0, 0x95A9D6FD)),
        "unit category stays education"
    );
    assert!(
        matches!(unit.get(0x0AD7FAC9).map(|p| &p.kind),
            Some(Kind::Scalar(Value::Float(v))) if (*v - 250.0).abs() < 1e-6),
        "student capacity written"
    );
    let bundles = |file: &PropertyFile, hash: u32| match file.get(hash).map(|p| &p.kind) {
        Some(Kind::Array(vs)) => vs
            .iter()
            .filter_map(|v| match v {
                Value::Key(k) => Some(k.instance),
                _ => None,
            })
            .collect::<Vec<u32>>(),
        _ => Vec::new(),
    };
    let union: Vec<u32> = [0x0A0209A2usize, 0x0A0209A5]
        .iter()
        .flat_map(|hash| bundles(&unit, *hash as u32))
        .collect();
    assert!(
        union.contains(&0x6C042367),
        "education bundle enabled: {union:08X?}"
    );
    assert!(
        union.contains(&0xE7690450),
        "donor tree bundle preserved: {union:08X?}"
    );

    // 5) Lot 看板字段落到教育看板。
    let lot = PropertyFile::parse(&read(at(0x00B1B104, 0x40E1C000))).unwrap();
    assert_eq!(
        key_instance(&lot, 0x0D7970ED),
        Some((0, 0x6A1785B4)),
        "lot board switched to education"
    );

    // 6) 确定性：同输入两次构建字节一致。
    let second = build_from_schema(&root, &schema).unwrap();
    assert_eq!(first.sha256, second.sha256, "deterministic output");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn behavior_requires_full_mode_and_allows_menu_combination() {
    let root = temp_root();
    behavior_donor(&root);
    let delta = build_from_schema(&root, &behavior_schema("delta").to_string())
        .err()
        .expect("behavior + delta must fail");
    assert!(
        delta.contains("full"),
        "error names the required mode: {delta}"
    );
    // 2026-11-07：menu + behavior 组合解禁——行为绑定到菜单阶段新分配的
    // Unit/Lot 身份（Menu2 冻结校验仍然生效），产出独立菜单入口 + 行为。
    let mut mixed = behavior_schema("full");
    mixed["semantics"]["menu"] = serde_json::json!({ "title": "combo-test" });
    // 新身份路线的参照必须是 lot-config（ground 输入类型化）。
    mixed["reference"]["lot"] = serde_json::json!("00B1B104:58F0C000:8D007745");
    let result = build_from_schema(&root, &mixed.to_string())
        .expect("behavior + menu combination must build");
    let output = Package::open(&result.path).unwrap();
    let menu_ids: Vec<u32> = output
        .entries()
        .iter()
        .filter(|entry| entry.id.type_id == 0x00B1B104 && entry.id.group == 0x09878A01)
        .map(|entry| entry.id.instance)
        .collect();
    assert!(
        menu_ids.iter().any(|instance| *instance != 0x8D007745),
        "menu combination allocates a new identity: {menu_ids:08X?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_unknown_scene_write_family_before_staging() {
    let root = temp_root();
    source_package(&root);
    let mut schema = schema();
    schema["scene"]["writes"] = serde_json::json!(["path"]);
    let error = build_from_schema(&root, &schema.to_string()).unwrap_err();
    assert!(error.contains("scene.writes"), "{error}");
    assert!(error.contains("props:<0..13>"), "{error}");
    assert!(!root.join(".build").exists());
    let _ = fs::remove_dir_all(root);
}

/// 2026-11-07 逻辑编排图端到端：document(menu+logic) → workbench_logic 编译
/// → schema 覆盖 → 构建。断言行为绑定到**新菜单身份**的 Unit 叶子上，
/// labor/core/education 的节点参数真实落到列。
#[test]
fn composed_logic_graph_binds_labor_and_education_to_new_menu_identity() {
    let root = temp_root();
    behavior_donor(&root);
    let document = serde_json::json!({
        "editor": {"workbench": {
            "menu": {
                "enabled": true,
                "label": "逻辑编排测试楼",
                "description": "菜单+行为组合",
                "uiPosition": 12000,
                "iconPng": "",
                "marqueePng": "",
                "constructionCost": null,
                "maintenanceCost": null,
                "entries": [
                    {"id": "menu-1", "dimension": "city", "categoryInstance": "899BEA70"}
                ]
            },
            "logic": {
                "nodes": [
                    {"id": "info-1", "kind": "building-info", "position": [0, 0]},
                    {"id": "core-1", "kind": "behavior-core", "position": [0, 120]},
                    {"id": "leaf-education-1", "kind": "behavior-leaf", "position": [0, 240]}
                ],
                "edges": [
                    {"id": "e1", "source": "info-1", "target": "asset", "targetHandle": "info"},
                    {"id": "e2", "source": "core-1", "target": "asset", "targetHandle": "behavior"},
                    {"id": "e3", "source": "leaf-education-1", "target": "core-1", "targetHandle": "leaf"}
                ],
                "params": {
                    "info-1": {"w1": 20, "w2": 5, "w3": 0, "minWorkers": 5},
                    "core-1": {
                        "power": {"mode": "explicit", "rate": 72, "amount": 90},
                        "water": {"mode": "inherit", "rate": null, "amount": null},
                        "garbage": {"mode": "inherit", "rate": null, "amount": null}
                    },
                    "leaf-education-1": {"type": "education", "capacity": 250}
                }
            }
        }}
    });
    fs::write(root.join("asset.lot.json"), document.to_string()).unwrap();
    let compiled = crate::asset_build_m1::workbench_logic::compile(&document, &|_| true).unwrap();
    assert!(compiled.menu.is_some());
    assert_eq!(compiled.behaviors.len(), 2);
    let mut schema = behavior_schema("full");
    // 逻辑图取代 legacy behavior：先移除旧绑定（两者并存是显式冲突）。
    schema["semantics"]
        .as_object_mut()
        .unwrap()
        .remove("behavior");
    crate::asset_build_m1::workbench_logic::apply_to_schema(&mut schema, &compiled).unwrap();
    let result = build_from_schema(&root, &schema.to_string()).unwrap();
    let instance = result.menu_instance.expect("menu identity allocated");
    assert_ne!(
        instance, 0x8D007745,
        "behavior must target the NEW identity"
    );

    let output = Package::open(&result.path).unwrap();
    let unit_id = ResourceId {
        type_id: 0x00B1B104,
        group: 0x40E0C000,
        instance,
    };
    let entry = output.entry(unit_id).expect("new Unit identity in output");
    let unit = PropertyFile::parse(&output.read(entry).unwrap()).unwrap();
    let float = |hash: u32| match unit.get(hash).map(|p| &p.kind) {
        Some(Kind::Scalar(Value::Float(value))) => Some(*value),
        _ => None,
    };
    assert_eq!(float(0x0A79_17C5), Some(20.0), "W1 jobs written");
    assert_eq!(float(0x0A79_17C6), Some(5.0), "W2 jobs written");
    assert_eq!(float(0x0A79_17C8), Some(5.0), "minimum workers written");
    assert_eq!(float(0x0AD7_FAC9), Some(250.0), "student capacity written");
    assert_eq!(float(0x0B88_904E), Some(90.0), "power amount written");
    assert_eq!(float(0x0C09_DAA0), Some(270.0), "power capacity derived 3x");

    // 菜单身份：新 Menu2 + locale 标题来自文档菜单绑定。
    let menu_id = ResourceId {
        type_id: 0x00B1B104,
        group: 0x09878A01,
        instance,
    };
    assert!(output.entry(menu_id).is_some(), "new Menu2 present");
    let _ = fs::remove_dir_all(root);
}
