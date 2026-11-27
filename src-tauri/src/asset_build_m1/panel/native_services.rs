use super::{PanelSchema, sources};
use dbpf::{Package, ResourceId};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub(super) const SCRIPT: ResourceId = ResourceId {
    type_id: 0x6777_1F5C,
    group: 0,
    instance: 0x3D93_2B6B,
};
pub(super) const CONSTANTS: ResourceId = ResourceId {
    type_id: 0x6777_1F5C,
    group: 0xA31E_060F,
    instance: 0x05AF_D67D,
};
pub(super) const RECIPES: &str =
    include_str!("../../../../src/lib/panel-native-service-items.json");

pub(super) fn recipes() -> BTreeMap<String, Vec<String>> {
    serde_json::from_str(RECIPES).expect("native panel service item contract")
}

fn has_binding(script: &str, name: &str) -> bool {
    let prefix = format!("simcity.RolloverItems.{name}");
    script.match_indices(&prefix).any(|(offset, _)| {
        script[offset + prefix.len()..]
            .trim_start()
            .starts_with('=')
    })
}

/// Stock and EP1 register these items outside the shared runtime receipt.
pub(crate) fn resolve_installed_bindings(
    game_data: &Path,
    stock: &[u8],
    bindings: &mut BTreeSet<u32>,
) -> Result<(), String> {
    let mut names: Vec<String> = recipes().into_values().flatten().collect();
    names.extend(
        [
            "kFusionReactorFeedback",
            "kARFTextFeedback",
            "kMaintenance_Unconnected",
        ]
        .map(str::to_owned),
    );
    let mut ep1 = None;
    for name in names {
        let id = super::hash(&name);
        if !bindings.contains(&id) {
            continue;
        }
        let stock = std::str::from_utf8(stock).map_err(|error| error.to_string())?;
        if !has_binding(stock, &name) {
            if ep1.is_none() {
                let path = game_data.join("SimCityDataEP1.package");
                let package =
                    Package::open(&path).map_err(|error| format!("{}: {error}", path.display()))?;
                let script = sources::read(std::slice::from_ref(&package), SCRIPT)?;
                sources::read(&[package], CONSTANTS)?;
                ep1 = Some(String::from_utf8(script).map_err(|error| error.to_string())?);
            }
            if !ep1
                .as_ref()
                .is_some_and(|script| has_binding(script, &name))
            {
                return Err(format!("PANEL_NATIVE_BINDING:{name}"));
            }
        }
        bindings.remove(&id);
    }
    Ok(())
}

pub(super) fn fusion_view(items: &[String]) -> bool {
    items.iter().any(|id| id.starts_with("fusion-"))
}

pub(super) fn feedback(items: &[String]) -> Option<(&str, &'static str)> {
    for (prefix, binding) in [
        ("fusion-", "kFusionReactorFeedback"),
        ("controlnet-", "kARFTextFeedback"),
    ] {
        if let Some(item) = items.iter().find(|id| id.starts_with(prefix)) {
            let owner = if items.iter().any(|id| id == "feedback") {
                "feedback"
            } else {
                item
            };
            return Some((owner, binding));
        }
    }
    None
}

pub(super) fn verify_sources(config: &PanelSchema, packages: &[Package]) -> Result<(), String> {
    let recipes = recipes();
    let selected: Vec<&str> = config
        .hover
        .iter()
        .chain(&config.click)
        .flat_map(|id| recipes.get(id).into_iter().flatten().map(String::as_str))
        .collect();
    if selected.is_empty() {
        return Ok(());
    }
    let bytes = sources::read(packages, SCRIPT)?;
    let script = std::str::from_utf8(&bytes).map_err(|error| error.to_string())?;
    sources::read(packages, CONSTANTS)?;
    for (_, binding) in [feedback(&config.hover), feedback(&config.click)]
        .into_iter()
        .flatten()
    {
        if !has_binding(script, binding) {
            return Err(format!("PANEL_NATIVE_BINDING:{binding}"));
        }
    }
    for (binding, layout) in [
        ("kFusionReactorPowerStored", "FusionProgress"),
        ("kFusionReactorPowerUp", "RolloverItemFusionButton"),
        ("kAcademyWorkers", "RolloverItemAcademySkillWorkers"),
        (
            "kAcademySkillGenerated",
            "RolloverBlockTextTitle2Row2Column",
        ),
        (
            "kAcademyConsumerCondensed",
            "RolloverBlockIconAndText1Row2Column",
        ),
        (
            "kAcademySkillForLocalUseToggle",
            "RolloverResourceNeighborTradeCheckBox",
        ),
    ] {
        if selected.contains(&binding) {
            if !has_binding(script, binding) {
                return Err(format!("PANEL_NATIVE_BINDING:{binding}"));
            }
            sources::read(
                packages,
                ResourceId {
                    type_id: SCRIPT.type_id,
                    group: 0x0B07_4E5A,
                    instance: super::hash(layout),
                },
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_service_controls_out_of_the_shared_runtime_receipt_only_when_installed() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("native-service-panel-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let stock = b"simcity.RolloverItems.kPowerForLocalUseToggle={};";
        let extension = super::super::hash("OpenSCPPanel_headerNone");
        let power = super::super::hash("kPowerForLocalUseToggle");
        let ignition = super::super::hash("kFusionReactorPowerUp");
        let mut bindings = BTreeSet::from([extension, power, ignition]);
        assert!(resolve_installed_bindings(&root, stock, &mut bindings).is_err());
        let ep1 = root.join("SimCityDataEP1.package");
        let bytes = dbpf::write_uncompressed_overlay(&[
            dbpf::OverlayEntry::new(
                SCRIPT,
                b"simcity.RolloverItems.kFusionReactorPowerUp = {};".to_vec(),
            ),
            dbpf::OverlayEntry::new(CONSTANTS, b"// fixture".to_vec()),
        ])
        .unwrap();
        std::fs::write(ep1, bytes).unwrap();
        resolve_installed_bindings(&root, stock, &mut bindings).unwrap();
        assert_eq!(bindings, BTreeSet::from([extension]));
        std::fs::remove_dir_all(root).unwrap();
    }
}
