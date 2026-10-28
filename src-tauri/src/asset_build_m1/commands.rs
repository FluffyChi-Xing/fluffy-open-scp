//! Opt-in entry point. Existing mod_flow/workbench commands are unchanged.
use super::{M1BuildResult, build_from_schema};
use crate::activity::CommandError;

#[tauri::command]
pub async fn asset_build_m1_build(
    root: String,
    schema: String,
) -> Result<M1BuildResult, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        build_from_schema(std::path::Path::new(&root), &schema)
    })
    .await
    .map_err(|e| CommandError::new("asset_build_m1_failed", e.to_string()))?
    .map_err(|e| CommandError::new("asset_build_m1_failed", e))
}
