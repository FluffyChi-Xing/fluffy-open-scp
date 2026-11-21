//! Actor preview IO: discover source packages and register only resolved mesh owners.
use crate::{
    activity::{AppState, CommandError},
    package_service::TgiDto,
};
use dbpf::Package;
use sc_properties::actor_model_chain::{ModelKey, Resolution, trace};
use serde::Deserialize;
use std::sync::Arc;
use tauri::State;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    package_id: u64,
    references: Vec<TgiDto>,
}
#[tauri::command]
pub async fn resolve_actor_models(
    state: State<'_, AppState>,
    request: Request,
) -> Result<Resolution, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        if request.references.len() > 32 {
            return Err(CommandError::internal("actor reference limit exceeded"));
        }
        let preferred = manager
            .get(request.package_id)
            .map_err(CommandError::from)?;
        let mut packages = manager
            .all_packages_with_ids()
            .map_err(CommandError::from)?;
        packages.sort_by_key(|(id, _)| (*id != request.package_id, *id));
        let mut dirs = vec![];
        if let Some(dir) = preferred.path().parent() {
            dirs.push(dir.to_path_buf());
        }
        if let Ok(Some(settings)) = store.app_settings() {
            if let Some(path) = settings.game_data_path {
                dirs.push(path.into());
            }
        }
        let mut paths = vec![];
        for dir in dirs {
            paths.extend(crate::game_assets::package_paths(&dir));
        }
        paths.sort();
        paths.dedup();
        // Keep unopened sources transient. Only packages owning resolved meshes are registered.
        let mut transient = vec![];
        for path in paths {
            if packages.iter().any(|(_, p)| p.path() == path) {
                continue;
            }
            if let Ok(pkg) = Package::open(&path) {
                transient.push(Arc::new(pkg));
            }
        }
        let base_len = packages.len();
        for (i, pkg) in transient.iter().enumerate() {
            packages.push((u64::MAX - i as u64, Arc::clone(pkg)));
        }
        let mut result = trace(
            &packages,
            request
                .references
                .into_iter()
                .map(|k| ModelKey {
                    type_id: k.type_id,
                    group: k.group,
                    instance: k.instance,
                })
                .collect(),
        );
        for model in &mut result.models {
            if let Some((_, pkg)) = packages[base_len..]
                .iter()
                .find(|(id, _)| *id == model.package_id)
            {
                model.package_id = manager
                    .open_reusable(pkg.path())
                    .map_err(CommandError::from)?;
            }
        }
        Ok(result)
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
}
