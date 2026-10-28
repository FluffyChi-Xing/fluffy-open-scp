//! Pure validation and lowering. No package reads or artifact evaluation.
use super::patch::{PropertyPatch, PropertyPatchSpec, parse_patch};
use crate::asset_build_m0::{IntermediateModel, lower_schema, parse_schema};
use dbpf::ResourceId;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Serialize)]
pub struct PropertyBuildPlan {
    pub processor_version: &'static str,
    pub scope: &'static str,
    pub intermediate: IntermediateModel,
    pub patches: Vec<PropertyPatchSpec>,
    pub order: [&'static str; 3],
    #[serde(skip)]
    pub(crate) typed_patches: Vec<PropertyPatch>,
    #[serde(skip)]
    pub(crate) protected: HashSet<ResourceId>,
}

pub fn plan_from_schema(raw: &str) -> Result<PropertyBuildPlan, String> {
    let parsed = parse_schema(raw).map_err(|error| {
        serde_json::to_string(&error.diagnostics).unwrap_or_else(|_| error.to_string())
    })?;
    let im = lower_schema(&parsed)?;
    if im.references.package.is_none() {
        return Err("M1 requires reference.package".into());
    }
    if im.scene.model.is_some()
        || !im.scene.slots.is_empty()
        || !im.semantics.unit_overrides.is_empty()
        || !im.semantics.lot_bindings.is_empty()
        || !im.semantics.menu.is_empty()
    {
        return Err("M1 supports explicit semantics.properties only; model, raster and semantic lowering are not implemented".into());
    }
    // Portable output names: avoid Windows devices, streams and trailing-dot aliases.
    let name = &im.output.package;
    let stem = name.trim_end_matches(".package");
    if stem.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        || name.starts_with('.')
        || [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ]
        .contains(
            &stem
                .split('.')
                .next()
                .unwrap_or("")
                .to_ascii_uppercase()
                .as_str(),
        )
    {
        return Err("M1 output must be a portable package filename".into());
    }
    let specs: Vec<PropertyPatchSpec> = im
        .semantics
        .properties
        .iter()
        .cloned()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(|error| format!("semantics.properties: {error}"))?;
    let patches = specs
        .iter()
        .cloned()
        .map(parse_patch)
        .collect::<Result<Vec<_>, _>>()?;
    let mut protected = HashSet::new();
    for tgi in [im.references.unit_template, im.references.lot_template]
        .into_iter()
        .flatten()
    {
        protected.insert(ResourceId {
            type_id: tgi.type_id,
            group: tgi.group,
            instance: tgi.instance,
        });
    }
    // Known landmark template pair stays protected even if omitted from reference.
    for group in [0x48E0C000, 0x40E1C000] {
        protected.insert(ResourceId {
            type_id: 0x00B1B104,
            group,
            instance: 0x4E01E311,
        });
    }
    for patch in &patches {
        if !im.semantics.allow_shared_template
            && (protected.contains(&patch.target) || patch.target.group == 0x48E0C000)
        {
            return Err(format!(
                "refusing to patch shared template {}",
                patch.target
            ));
        }
    }
    Ok(PropertyBuildPlan {
        processor_version: "openscp.property.m1/1",
        scope: "property-only",
        intermediate: im,
        patches: specs,
        typed_patches: patches,
        protected,
        order: ["property", "dbpf", "readback"],
    })
}
