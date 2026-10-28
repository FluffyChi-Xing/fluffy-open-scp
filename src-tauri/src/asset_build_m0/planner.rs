use crate::asset_build_m0::diagnostics::DiagnosticSet;
use crate::asset_build_m0::ir::{IntermediateModel, ProcessorKind, ProcessorSlot};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanManifest {
    pub version: u32,
    pub schema_id: String,
    pub schema_hash: String,
    pub build_hash: String,
    pub order: Vec<String>,
    pub artifacts: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDescriptor {
    pub id: String,
    pub kind: ProcessorKind,
    pub output: String,
    pub input: Option<String>,
}

/// A one-shot, side-effect boundary. Planning never evaluates this closure.
pub type LazyArtifact = Box<dyn FnOnce() -> Result<Vec<u8>, String> + Send>;

pub struct PlannedArtifact {
    pub descriptor: ArtifactDescriptor,
    pub materialize: LazyArtifact,
}

pub struct BuildPlan {
    pub intermediate: IntermediateModel,
    pub manifest: PlanManifest,
    pub diagnostics: DiagnosticSet,
    pub artifacts: Vec<PlannedArtifact>,
}

pub fn plan_build(intermediate: IntermediateModel) -> Result<BuildPlan, String> {
    let order = topological_order(&intermediate.slots)?;
    let build_hash = build_hash(&intermediate, &order)?;
    let descriptors: Vec<_> = order
        .iter()
        .filter_map(|id| intermediate.slots.iter().find(|slot| &slot.id == id))
        .map(describe)
        .collect();
    let artifacts = descriptors
        .iter()
        .cloned()
        .map(|descriptor| {
            let snapshot = serde_json::to_vec(&descriptor).map_err(|error| error.to_string());
            PlannedArtifact {
                descriptor,
                materialize: Box::new(move || snapshot.clone().map_err(|error| error)),
            }
        })
        .collect();
    Ok(BuildPlan {
        manifest: PlanManifest {
            version: 1,
            schema_id: intermediate.schema_id.clone(),
            schema_hash: intermediate.schema_hash.clone(),
            build_hash,
            order,
            artifacts: descriptors,
        },
        intermediate,
        diagnostics: DiagnosticSet::default(),
        artifacts,
    })
}

fn describe(slot: &ProcessorSlot) -> ArtifactDescriptor {
    let extension = match slot.kind {
        ProcessorKind::Property => "property",
        ProcessorKind::Raster => "raster",
        ProcessorKind::Rw4 => "rw4",
        ProcessorKind::Menu => "menu",
        ProcessorKind::Package => "package",
    };
    ArtifactDescriptor {
        id: slot.id.clone(),
        kind: slot.kind.clone(),
        output: format!("artifacts/{}/{extension}", slot.id.replace(':', "_")),
        input: slot.input.clone(),
    }
}

fn build_hash(intermediate: &IntermediateModel, order: &[String]) -> Result<String, String> {
    let bytes = serde_json::to_vec(&(intermediate, order)).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn topological_order(slots: &[ProcessorSlot]) -> Result<Vec<String>, String> {
    // M0 slots are already lowered in dependency order. The explicit check
    // keeps the planner deterministic and leaves room for graph edges in M1.
    let mut order = Vec::with_capacity(slots.len());
    for slot in slots {
        if order.iter().any(|id| id == &slot.id) {
            return Err(format!("duplicate processor slot {}", slot.id));
        }
        order.push(slot.id.clone());
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset_build_m0::{lower_schema, parse_schema};

    #[test]
    fn planning_is_deterministic_and_lazy() {
        let raw = serde_json::json!({
            "$schema": "openscp.asset", "schema_version": 2,
            "id": "asset", "kind": "building-asset",
            "reference": {}, "scene": {"document": "asset.lot.json"},
            "outputs": {"package": "asset.package"}
        });
        let parsed = parse_schema(&raw.to_string()).unwrap();
        let plan = plan_build(lower_schema(&parsed).unwrap()).unwrap();
        assert_eq!(
            plan.manifest.build_hash,
            plan_build(plan.intermediate.clone())
                .unwrap()
                .manifest
                .build_hash
        );
        let artifact = plan.artifacts.into_iter().next().unwrap();
        assert!(artifact.descriptor.id == "package");
        assert!(!(artifact.materialize)().unwrap().is_empty());
    }
}
