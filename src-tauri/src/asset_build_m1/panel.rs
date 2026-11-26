//! Opt-in building panel compiler. Absence of dependencies.panel is a strict no-op.
pub mod commands;
pub(super) mod container_contract;
mod container_shoppers;
mod component_queries;
mod tower_operations;
mod layout;
mod ledger;
pub mod manifest;
mod processor;
mod runtime_registry;
pub mod runtime_bundle;
mod sources;
mod spec;
pub(crate) use processor::{apply, attach};
pub(super) use tower_operations::verify_single_business;
pub(super) use component_queries::verify_written_service_controls;
pub(crate) use sources::GAME_JS as PANEL_GAME_JS;
pub(crate) use spec::PanelPlan;
pub use spec::{Component, PanelSchema, components, parse};
pub(crate) const RUNTIME_OUTPUT: &str = "dependencies/openscp-panel-runtime-v1.package";
pub const SHARED_RUNTIME_NAME: &str = "openscp-panel-runtime-v1.package";
/// Export a single installation-specific runtime outside the game directory.
/// Asset builds carry their required binding IDs in the authored Asset.
pub fn export_shared_runtime(game_data: &std::path::Path, output: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let game_data = game_data.canonicalize().map_err(|e| e.to_string())?;
    let output = output.canonicalize().map_err(|e| e.to_string())?;
    let installation = game_data.parent().unwrap_or(&game_data);
    if !output.is_dir() || output.starts_with(installation) {
        return Err("PANEL_EXPORT_OUTSIDE_GAME".into());
    }
    let base = game_data.join("SimCity_Game.package");
    let stock = runtime_bundle::read_package(&base)?;
    validate_stock_bindings(&stock)?;
    let runtime = runtime_bundle::compose(&stock)?;
    let mut resources = super::connector::Resources::new();
    resources.insert(sources::GAME_JS, runtime);
    let bytes = super::connector::link(&resources)?;
    let destination = output.join(SHARED_RUNTIME_NAME);
    if let Ok(meta) = std::fs::symlink_metadata(&destination) {
        if !meta.is_file() {
            return Err("PANEL_RUNTIME_EXPORT_CONFLICT".into());
        }
        let installed = runtime_bundle::read_package(&destination)?;
        runtime_bundle::verify_source(&stock, &installed)?;
        super::connector::verify(&destination, &[(sources::GAME_JS, installed)].into_iter().collect())?;
        crate::atomic_fs::write_atomic(&destination, &bytes, true).map_err(|e| e.to_string())?;
    } else {
        crate::atomic_fs::write_atomic(&destination, &bytes, false).map_err(|e| e.to_string())?;
    }
    super::connector::verify(&destination, &resources)?;
    Ok(destination)
}

fn validate_stock_bindings(stock: &[u8]) -> Result<(), String> {
    let stock = std::str::from_utf8(stock).map_err(|e| e.to_string())?;
    if stock.contains("OpenSCPPanelRuntimeVersion") {
        return Err("PANEL_RUNTIME_SOURCE_ALREADY_PATCHED".into());
    }
    for component in components().into_iter().filter(|item| !item.tower_operations) {
        if !stock.contains(&format!("simcity.RolloverItems.{}", component.native)) {
            return Err(format!("PANEL_NATIVE_BINDING:{}", component.native));
        }
    }
    Ok(())
}
pub(crate) const RUNTIME: &str =
    include_str!("../../../src/pages/studio/components/asset/panel-editor/native/runtime.js");
pub(crate) const MESSAGES: &str = include_str!("../../../src/lib/panel-messages.json");

pub fn hash(name: &str) -> u32 {
    name.bytes().fold(2_166_136_261, |h, b| {
        h.wrapping_mul(16_777_619) ^ u32::from(b.to_ascii_lowercase())
    })
}
pub fn item_name(id: &str) -> String {
    format!("OpenSCPPanel_{id}")
}
#[cfg(test)]
mod tests;
