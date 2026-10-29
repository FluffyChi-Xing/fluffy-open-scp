//! Application orchestration; processors and the connector own their formats.
use super::{
    connector,
    filesystem::{Staging, project_file},
    plan::plan_from_schema,
    processor::defer_property,
};
use dbpf::ResourceId;
use sc_properties::{Key, Kind, PropertyFile, Value};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
pub struct M1BuildResult {
    pub path: PathBuf,
    pub schema_hash: String,
    pub build_hash: String,
    pub source_hash: String,
    pub sha256: String,
    pub resource_count: usize,
    pub patched_resources: usize,
    pub scope: &'static str,
    pub scene: SceneSemanticSummary,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SceneSemanticSummary {
    pub unit_count: usize,
    pub path_count: usize,
    pub path_entry_count: usize,
}

pub fn build_from_schema(root: &Path, raw_schema: &str) -> Result<M1BuildResult, String> {
    let plan = plan_from_schema(raw_schema)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if !root.is_dir() {
        return Err("project root must be a directory".into());
    }
    let source_path = project_file(
        &root,
        plan.intermediate
            .references
            .package
            .as_deref()
            .ok_or("missing source package")?,
    )?;
    let mut resources = connector::read_source(&source_path)?;
    let mut texture_instances = [0u32; 6];
    for (slot, path) in &plan.intermediate.scene.slots {
        let index = slot
            .strip_prefix("slot")
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or_else(|| format!("invalid raster slot {slot}"))?;
        if index >= texture_instances.len() {
            return Err(format!("raster slot out of range: {slot}"));
        }
        let path = project_file(&root, path)?;
        let bytes = std::fs::read(&path).map_err(|e| format!("raster slot {slot}: {e}"))?;
        let digest = Sha256::digest(&bytes);
        let instance = u32::from_be_bytes(digest[..4].try_into().unwrap());
        let id = ResourceId {
            type_id: 0x2F4E_681C,
            group: 0,
            instance,
        };
        resources.insert(id, bytes);
        texture_instances[index] = instance;
    }
    let native_model = if let Some(model_path) = &plan.intermediate.scene.model {
        let model_path = project_file(&root, model_path)?;
        let bytes = std::fs::read(&model_path).map_err(|e| format!("scene model: {e}"))?;
        let compiled =
            super::native_model::compile_lotm_static_with_textures(&bytes, &texture_instances)?;
        let mut instance_bytes = Sha256::digest(&compiled.rw4);
        let mut instance = u32::from_be_bytes(instance_bytes[..4].try_into().unwrap());
        while resources.contains_key(&ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance,
        }) {
            instance = instance.wrapping_add(1);
        }
        let id = ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance,
        };
        resources.insert(id, compiled.rw4);
        Some(id)
    } else {
        None
    };
    let scene =
        validate_scene_path_semantics(&root, &plan.intermediate.scene.document, &resources)?;
    // Hash the decompressed snapshot actually consumed by the processors, not a
    // second file read. Container compression differences do not change identity.
    let source_hash = connector::fingerprint(&resources);
    let plan_bytes = serde_json::to_vec_pretty(&plan).map_err(|e| e.to_string())?;
    let build_hash = digest(
        &serde_json::to_vec(&(plan.processor_version, &source_hash, &plan_bytes))
            .map_err(|e| e.to_string())?,
    );
    let protected = protected_templates(&resources, &plan.protected);
    let mut targets: Vec<_> = plan.typed_patches.iter().map(|p| p.target).collect();
    targets.sort_by_key(|id| (id.type_id, id.group, id.instance));
    targets.dedup();
    let mut tasks = Vec::new();
    for target in &targets {
        if !plan.intermediate.semantics.allow_shared_template && protected.contains(target) {
            return Err(format!("refusing to patch shared template {target}"));
        }
        let source = resources
            .get(target)
            .ok_or_else(|| format!("missing Property target {target}"))?;
        let patches = plan
            .typed_patches
            .iter()
            .filter(|p| p.target == *target)
            .cloned()
            .collect();
        tasks.push(defer_property(*target, source.clone(), patches));
    }
    // No output directories are created until every Property processor succeeds.
    for task in tasks {
        let bytes = (task.materialize)().map_err(|e| format!("{}: {e}", task.target))?;
        resources.insert(task.target, bytes);
    }
    if let Some(model_id) = native_model {
        for target in [
            plan.intermediate.references.unit,
            plan.intermediate.references.lot,
        ]
        .into_iter()
        .flatten()
        .map(|tgi| ResourceId {
            type_id: tgi.type_id,
            group: tgi.group,
            instance: tgi.instance,
        }) {
            if let Some(bytes) = resources.get(&target).cloned() {
                let mut file = PropertyFile::parse(&bytes)
                    .map_err(|e| format!("model property {target}: {e}"))?;
                for hash in [
                    sc_properties::LOD1_MODEL_HASH,
                    0x00F9_EFBC,
                    0x00F9_EFBD,
                    0x00F9_EFBE,
                ] {
                    if let Some(property) = file.values.iter_mut().find(|p| p.hash == hash) {
                        if let Kind::Scalar(Value::Key(key)) = &mut property.kind {
                            key.type_id = model_id.type_id;
                            key.group = model_id.group;
                            key.instance = model_id.instance;
                        }
                    }
                }
                resources.insert(
                    target,
                    file.encode_canonical()
                        .map_err(|e| format!("model property encode {target}: {e}"))?,
                );
            }
        }
    }
    let output = connector::link(&resources)?;
    let mut staging = Staging::create(&root, &build_hash)?;
    let package_relative = format!("packages/{}", plan.intermediate.output.package);
    staging.write("plan/plan.json", &plan_bytes)?;
    staging.write(
        "plan/im.json",
        &serde_json::to_vec_pretty(&plan.intermediate).map_err(|e| e.to_string())?,
    )?;
    let scene_path = project_file(&root, &plan.intermediate.scene.document)?;
    if scene_path.is_file() {
        let scene_bytes =
            std::fs::read(&scene_path).map_err(|error| format!("scene document: {error}"))?;
        staging.write("artifacts/scene/asset.lot.json", &scene_bytes)?;
    }
    materialize_scene_resources(&root, &plan.intermediate, &resources, &mut staging)?;
    for target in &targets {
        let name = format!(
            "artifacts/property/{:08X}-{:08X}-{:08X}.property",
            target.type_id, target.group, target.instance
        );
        staging.write(&name, &resources[target])?;
    }
    staging.write(&package_relative, &output)?;
    connector::verify(&staging.path.join(&package_relative), &resources)?;
    let result = M1BuildResult {
        path: staging.destination.join(&package_relative),
        schema_hash: plan.intermediate.schema_hash,
        build_hash,
        source_hash,
        sha256: digest(&output),
        resource_count: resources.len(),
        patched_resources: targets.len(),
        scope: "property-only",
        scene,
    };
    staging.write(
        "reports/m1-build.json",
        &serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )?;
    staging.publish()?;
    Ok(result)
}

#[derive(Debug, Clone, Default, Serialize)]
struct SceneResourceManifest {
    lot: Vec<String>,
    prop: Vec<String>,
    bin: Vec<String>,
    path: Vec<String>,
    path_entry: Vec<String>,
    raster: Vec<String>,
    missing: Vec<String>,
}

fn materialize_scene_resources(
    root: &Path,
    intermediate: &crate::asset_build_m0::IntermediateModel,
    resources: &connector::Resources,
    staging: &mut Staging,
) -> Result<(), String> {
    let mut manifest = SceneResourceManifest::default();
    let mut requested: Vec<(&str, ResourceId)> = Vec::new();
    if let Some(lot) = intermediate.references.lot {
        requested.push((
            "lot",
            ResourceId {
                type_id: lot.type_id,
                group: lot.group,
                instance: lot.instance,
            },
        ));
    }
    let candidate = root.join(&intermediate.scene.document);
    if candidate.is_file() {
        let scene_path = project_file(root, &intermediate.scene.document)?;
        let value: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&scene_path).map_err(|error| format!("scene document: {error}"))?,
        )
        .map_err(|error| format!("scene document is not valid JSON: {error}"))?;
        if let Some(units) = value
            .pointer("/units")
            .and_then(serde_json::Value::as_array)
        {
            for unit in units {
                let kind = match unit.get("kind").and_then(serde_json::Value::as_str) {
                    Some("bin") => "bin",
                    _ => "prop",
                };
                if matches!(kind, "prop" | "bin") {
                    if let Some(tgi) = unit.get("modelTgi").and_then(parse_json_tgi) {
                        requested.push((kind, tgi));
                    }
                }
            }
        }
        if let Some(paths) = value
            .pointer("/editor/workbench/paths")
            .and_then(serde_json::Value::as_array)
        {
            for path in paths {
                if let Some(tgi) = path
                    .get("source")
                    .and_then(|source| source.get("tgi"))
                    .or_else(|| path.get("source"))
                    .and_then(parse_json_tgi)
                {
                    requested.push(("path", tgi));
                }
                if let Some(entry) = path
                    .get("entry")
                    .and_then(serde_json::Value::as_str)
                    .and_then(parse_hex_u32)
                {
                    requested.push((
                        "path_entry",
                        ResourceId {
                            type_id: 0x00B1_B104,
                            group: 0x40E0_C400,
                            instance: entry,
                        },
                    ));
                }
            }
        }
    }
    let targets: Vec<_> = intermediate
        .semantics
        .properties
        .iter()
        .filter_map(|value| value.get("tgi").and_then(parse_json_tgi))
        .collect();
    for id in intermediate
        .references
        .lot
        .into_iter()
        .map(|tgi| ResourceId {
            type_id: tgi.type_id,
            group: tgi.group,
            instance: tgi.instance,
        })
        .chain(targets)
    {
        if let Some((_, bytes)) = find_resource(resources, id) {
            if let Ok(file) = PropertyFile::parse(bytes) {
                if let Some(key) = file.get(0x0CCB_7FD5).and_then(property_key) {
                    requested.push((
                        "raster",
                        ResourceId {
                            type_id: 0x2F4E_681C,
                            group: key.group,
                            instance: key.instance,
                        },
                    ));
                }
            }
        }
    }
    requested.sort_by_key(|(_, id)| (id.type_id, id.group, id.instance));
    requested.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    for (kind, id) in requested {
        let key = format!("{:08X}-{:08X}-{:08X}", id.type_id, id.group, id.instance);
        let Some((resolved_id, bytes)) = find_resource(resources, id) else {
            manifest.missing.push(format!("{kind}:{key}"));
            continue;
        };
        let resolved_key = format!(
            "{:08X}-{:08X}-{:08X}",
            resolved_id.type_id, resolved_id.group, resolved_id.instance
        );
        staging.write(&format!("artifacts/{kind}/{resolved_key}.resource"), bytes)?;
        match kind {
            "lot" => manifest.lot.push(resolved_key),
            "prop" => manifest.prop.push(resolved_key),
            "bin" => manifest.bin.push(resolved_key),
            "path" => manifest.path.push(resolved_key),
            "path_entry" => manifest.path_entry.push(resolved_key),
            "raster" => manifest.raster.push(resolved_key),
            _ => {}
        }
    }
    staging.write(
        "artifacts/resource-manifest.json",
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )?;
    Ok(())
}

fn find_resource<'a>(
    resources: &'a connector::Resources,
    id: ResourceId,
) -> Option<(ResourceId, &'a Vec<u8>)> {
    resources
        .get_key_value(&id)
        .map(|(resolved, bytes)| (*resolved, bytes))
        .or_else(|| {
            resources
                .iter()
                .find(|(candidate, _)| {
                    candidate.type_id == id.type_id && candidate.instance == id.instance
                })
                .map(|(resolved, bytes)| (*resolved, bytes))
        })
}

fn property_key(property: &sc_properties::Property) -> Option<Key> {
    match &property.kind {
        Kind::Scalar(Value::Key(key)) => Some(*key),
        _ => None,
    }
}

fn validate_scene_path_semantics(
    root: &Path,
    document: &str,
    resources: &connector::Resources,
) -> Result<SceneSemanticSummary, String> {
    let candidate = root.join(document);
    if !candidate.is_file() {
        return Ok(SceneSemanticSummary::default());
    }
    let path = super::filesystem::project_file(root, document)?;
    let bytes = std::fs::read(&path).map_err(|error| format!("scene document: {error}"))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("scene document is not valid JSON: {error}"))?;
    let Some(paths) = value
        .pointer("/editor/workbench/paths")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(SceneSemanticSummary::default());
    };
    let mut errors = Vec::new();
    let unit_count = value
        .pointer("/units")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let mut path_entry_count = 0;
    for (index, path) in paths.iter().enumerate() {
        let location = format!("editor.workbench.paths[{index}]");
        let Some(entry_text) = path.get("entry").and_then(serde_json::Value::as_str) else {
            errors.push(format!("{location}.entry: missing path entry"));
            continue;
        };
        path_entry_count += 1;
        let Some(entry_instance) = parse_hex_u32(entry_text) else {
            errors.push(format!("{location}.entry: invalid path entry {entry_text}"));
            continue;
        };
        let Some(tgi) = path
            .get("source")
            .and_then(|source| source.get("tgi"))
            .or_else(|| path.get("source"))
        else {
            errors.push(format!("{location}.source.tgi: missing path source"));
            continue;
        };
        let Some(source_tgi) = parse_json_tgi(tgi) else {
            errors.push(format!("{location}.source.tgi: invalid path source"));
            continue;
        };
        let source = resources.get(&source_tgi).or_else(|| {
            resources
                .iter()
                .find(|(id, _)| {
                    id.type_id == source_tgi.type_id && id.instance == source_tgi.instance
                })
                .map(|(_, bytes)| bytes)
        });
        let Some(source) = source else {
            errors.push(format!(
                "{location}.source.tgi: source property {source_tgi} is missing"
            ));
            continue;
        };
        let Ok(file) = sc_properties::PropertyFile::parse(source) else {
            errors.push(format!("{location}.source.tgi: source property is invalid"));
            continue;
        };
        let bound = file.get(0x0CC8_FE7A).is_some_and(|property| match &property.kind {
            sc_properties::Kind::Scalar(sc_properties::Value::Key(key)) => key.instance == entry_instance,
            sc_properties::Kind::Array(values) => values.iter().any(|value| matches!(value, sc_properties::Value::Key(key) if key.instance == entry_instance)),
            _ => false,
        });
        if !bound {
            errors.push(format!(
                "{location}.entry: {entry_text} is not bound by source property 0x0CC8FE7A"
            ));
        }
        let resource_exists = resources.keys().any(|id| {
            id.type_id == 0x00B1_B104
                && id.instance == entry_instance
                && (id.group & 0xFFFF) == 0xC400
        });
        if !resource_exists {
            errors.push(format!(
                "{location}.entry: PathEntry resource instance 0x{entry_instance:08X} is missing"
            ));
        }
    }
    if errors.is_empty() {
        Ok(SceneSemanticSummary {
            unit_count,
            path_count: paths.len(),
            path_entry_count,
        })
    } else {
        Err(format!(
            "path semantic validation failed: {}",
            errors.join("; ")
        ))
    }
}

fn parse_hex_u32(value: &str) -> Option<u32> {
    u32::from_str_radix(
        value
            .trim()
            .trim_start_matches("0x")
            .trim_start_matches("0X"),
        16,
    )
    .ok()
}

fn parse_json_u32(value: &serde_json::Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .or_else(|| {
            value
                .as_str()
                .and_then(|value| value.parse().ok().or_else(|| parse_hex_u32(value)))
        })
}

fn parse_json_tgi(value: &serde_json::Value) -> Option<ResourceId> {
    Some(ResourceId {
        type_id: parse_json_u32(value.get("typeId")?)?,
        group: parse_json_u32(value.get("group")?)?,
        instance: parse_json_u32(value.get("instance")?)?,
    })
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn protected_templates(
    resources: &connector::Resources,
    declared: &HashSet<ResourceId>,
) -> HashSet<ResourceId> {
    let mut result = declared.clone();
    for (id, bytes) in resources {
        if id.type_id != sc_properties::PROPERTY_RESOURCE_TYPE {
            continue;
        }
        let Ok(file) = PropertyFile::parse(bytes) else {
            continue;
        };
        let Some(parent) = file.get(sc_properties::PARENT_HASH) else {
            continue;
        };
        let values = match &parent.kind {
            Kind::Scalar(v) => std::slice::from_ref(v),
            Kind::Array(v) => v.as_slice(),
            Kind::Empty => &[],
        };
        for value in values {
            if let Value::Key(key) = value {
                result.insert(ResourceId {
                    type_id: key.type_id,
                    group: key.group,
                    instance: key.instance,
                });
            }
        }
    }
    result
}
