//! Bin declarations (07E0AD23) from the original 40E0C700 bundle properties.
//! Compare bundles owned by separate leaves. Shared references to the same
//! bundle are intentional, as are shared bins within a single native recipe.

use super::behavior_nodes::{compile, parse_nodes};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
pub(super) struct Catalog {
    bundles: BTreeMap<String, Vec<u32>>,
    names: BTreeMap<u32, String>,
}

impl Catalog {
    pub(super) fn has_bundle(&self, id: u32) -> bool {
        self.bins(id).is_some()
    }

    fn bins(&self, id: u32) -> Option<&[u32]> {
        self.bundles.get(&format!("{id:08X}")).map(Vec::as_slice)
    }
}

pub(super) fn catalog() -> Result<Catalog, String> {
    serde_json::from_str(include_str!(
        "../../resources/sc5/behavior-bin-catalog.json"
    ))
    .map_err(|error| format!("BEHAVIOR_BIN_CATALOG: {error}"))
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(super) struct Conflict {
    other_id: String,
    bin: String,
    name: Option<String>,
    added_bundle: String,
    other_bundle: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct Report {
    conflicts: Vec<Conflict>,
    unknown_bundles: Vec<String>,
}

fn bundles(node: &Value) -> Result<BTreeSet<u32>, String> {
    let program = compile(None, std::slice::from_ref(node))?;
    let mut bundles: BTreeSet<_> = program
        .ensure_bundles
        .into_iter()
        .chain(program.templates.bundles())
        .chain(program.hotel.iter().flat_map(|recipe| recipe.bundles()))
        .collect();
    if node["type"] == "park" {
        // The generic-park donor supplies the visitors' bins and sinks.
        bundles.extend([0x6EA8_E701, 0x265F_B8DD]);
    }
    Ok(bundles)
}

pub(super) fn check(nodes: &[Value], added_id: &str) -> Result<Report, String> {
    let nodes = parse_nodes(nodes)?;
    let added = nodes
        .iter()
        .find(|node| node["id"] == added_id)
        .ok_or("BEHAVIOR_BIN_CHECK: missing added leaf")?;
    let added_bundles = bundles(added)?;
    let catalog = catalog()?;
    let mut report = Report::default();
    let mut unknown = BTreeSet::new();
    for &bundle in &added_bundles {
        if !catalog.has_bundle(bundle) {
            unknown.insert(format!("{bundle:08X}"));
        }
    }
    for other in nodes.iter().filter(|node| node["id"] != added_id) {
        let other_id = other["id"]
            .as_str()
            .ok_or("BEHAVIOR_BIN_CHECK: missing leaf id")?;
        let other_bundles = bundles(other)?;
        let mut collisions = BTreeMap::new();
        for other_bundle in other_bundles.iter().copied() {
            let Some(other_bins) = catalog.bins(other_bundle) else {
                unknown.insert(format!("{other_bundle:08X}"));
                continue;
            };
            for added_bundle in added_bundles.iter().copied() {
                if added_bundle == other_bundle
                    || (other_bundles.contains(&added_bundle)
                        && added_bundles.contains(&other_bundle))
                {
                    continue;
                }
                let Some(added_bins) = catalog.bins(added_bundle) else {
                    continue;
                };
                for bin in added_bins
                    .iter()
                    .copied()
                    .filter(|bin| other_bins.contains(bin))
                {
                    collisions
                        .entry(bin)
                        .or_insert((added_bundle, other_bundle));
                }
            }
        }
        report.conflicts.extend(
            collisions
                .into_iter()
                .map(|(bin, (added, other))| Conflict {
                    other_id: other_id.into(),
                    bin: format!("{bin:08X}"),
                    name: catalog.names.get(&bin).cloned(),
                    added_bundle: format!("{added:08X}"),
                    other_bundle: format!("{other:08X}"),
                }),
        );
    }
    report.unknown_bundles = unknown.into_iter().collect();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reports_real_open_shopper_overlap_between_park_and_tourism() {
        let nodes = [
            json!({"id":"park","type":"park","params":{}}),
            json!({"id":"tourism","type":"tourism","params":{"tourismProfile":"native-tourism-v1"}}),
        ];
        let result = check(&nodes, "tourism").unwrap();
        assert!(result.conflicts.iter().any(|row| {
            row.other_id == "park"
                && row.name.as_deref() == Some("OpenShoppers")
                && row.other_bundle == "6EA8E701"
        }));
    }

    #[test]
    fn ignores_shared_reference_and_overlaps_inside_one_recipe() {
        let nodes = [
            json!({"id":"tourism1","type":"tourism","params":{"tourismProfile":"native-tourism-v1"}}),
            json!({"id":"tourism2","type":"tourism","params":{"tourismProfile":"native-tourism-v1"}}),
        ];
        let result = check(&nodes, "tourism1").unwrap();
        assert!(result.conflicts.is_empty());
        assert!(result.unknown_bundles.is_empty());
    }
}
