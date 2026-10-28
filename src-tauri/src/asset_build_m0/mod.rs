//! M0 asset build foundation.
//!
//! This module is intentionally isolated from the existing asset workbench and
//! `mod_flow` command. It owns the new schema-v2 contract, typed intermediate
//! model, deterministic lazy plan, diagnostics and `.build` staging layout.
//! Format processors and package connectors will be added behind these seams in
//! later milestones.

pub mod diagnostics;
mod ir;
mod layout;
mod planner;
mod schema;

#[allow(unused_imports)]
pub use ir::{
    IntermediateModel, OutputIr, ProcessorKind, ProcessorSlot, ReferenceIr, SceneIr, SemanticsIr,
    Tgi, lower_schema,
};
#[allow(unused_imports)]
pub use layout::{BuildLayout, BuildRecord};
#[allow(unused_imports)]
pub use planner::{ArtifactDescriptor, BuildPlan, PlanManifest, PlannedArtifact, plan_build};
#[allow(unused_imports)]
pub use schema::{
    ASSET_SCHEMA, ASSET_SCHEMA_VERSION, AssetSchemaV2, OutputSchema, ParsedSchema, ReferenceSchema,
    SceneSchema, SemanticsSchema, parse_schema, parse_tgi, validate_schema,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m0_pipeline_is_schema_to_im_to_plan() {
        let source = serde_json::json!({
            "$schema": "openscp.asset", "schema_version": 2,
            "id": "willis-tower", "kind": "building-asset",
            "reference": {
                "menu": "00B1B104:09878A01:74168A83",
                "unit": "00B1B104:40E0C000:8D007745",
                "lot": "00B1B104:40E1C000:8D007745",
                "unitTemplate": "00B1B104:48E0C000:4E01E311",
                "lotTemplate": "00B1B104:40E1C000:4E01E311"
            },
            "scene": {
                "document": "asset.lot.json",
                "model": "assets/models/willis.lotm",
                "slots": {"slot0": "schemas/assets/texture-input-1.json"}
            },
            "semantics": {"class": "landmark", "menu": {"category": "3F43E038"}},
            "outputs": {"package": "willis-tower.package"}
        });
        let parsed = parse_schema(&source.to_string()).unwrap();
        let im = lower_schema(&parsed).unwrap();
        let plan = plan_build(im).unwrap();
        assert_eq!(plan.manifest.schema_id, "willis-tower");
        assert!(plan.manifest.order.contains(&"property".into()));
        assert!(plan.manifest.order.contains(&"rw4:model".into()));
        assert!(plan.manifest.order.contains(&"package".into()));
    }
}
