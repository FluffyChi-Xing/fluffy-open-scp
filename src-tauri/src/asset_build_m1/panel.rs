//! Opt-in building panel compiler. Absence of dependencies.panel is a strict no-op.
pub mod commands;
pub mod manifest;
mod processor;
mod sources;
mod spec;
pub(crate) use processor::{apply, attach};
pub(crate) use spec::PanelPlan;
pub use spec::{Component, PanelSchema, components, parse};
pub(crate) const RUNTIME_OUTPUT: &str = "dependencies/openscp-panel-runtime-v1.package";
pub(crate) fn publish_runtime(
    staging: &mut super::filesystem::Staging,
    runtime: &[u8],
) -> Result<(), String> {
    let mut resources = super::connector::Resources::new();
    resources.insert(sources::GAME_JS, runtime.to_vec());
    let bytes = super::connector::link(&resources)?;
    staging.write(RUNTIME_OUTPUT, &bytes)?;
    super::connector::verify(&staging.path.join(RUNTIME_OUTPUT), &resources)
}
pub(crate) const RUNTIME: &str =
    include_str!("../../../src/pages/studio/components/asset/panel-editor/native/runtime.js");

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
