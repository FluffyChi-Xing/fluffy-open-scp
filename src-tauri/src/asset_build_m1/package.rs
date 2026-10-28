//! Application orchestration; processors and the connector own their formats.
use super::{
    connector,
    filesystem::{Staging, project_file},
    plan::plan_from_schema,
    processor::defer_property,
};
use dbpf::ResourceId;
use sc_properties::{Kind, PropertyFile, Value};
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
    let output = connector::link(&resources)?;
    let staging = Staging::create(&root, &build_hash)?;
    let package_relative = format!("packages/{}", plan.intermediate.output.package);
    staging.write("plan/plan.json", &plan_bytes)?;
    staging.write(
        "plan/im.json",
        &serde_json::to_vec_pretty(&plan.intermediate).map_err(|e| e.to_string())?,
    )?;
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
    };
    staging.write(
        "reports/m1-build.json",
        &serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )?;
    staging.publish()?;
    Ok(result)
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
