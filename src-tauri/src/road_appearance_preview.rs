use crate::activity::{AppState, CommandError};
use crate::package_service::{TgiDto, locate_sim_source_packages};
use sc_properties::map_assets::{RoadAssets, road_appearance};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn read_road_appearance(
    state: State<'_, AppState>,
    package_id: u64,
    tgi: TgiDto,
) -> Result<RoadAssets, CommandError> {
    if tgi.type_id != 0x00B1_B104 || tgi.group != 0x0955_8B7E {
        return Err(CommandError::internal("road_appearance_type"));
    }
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let selected = manager.get(package_id)?;
        if selected.entry(tgi.clone().into()).is_none() {
            return Err(CommandError::internal("road_appearance_missing"));
        }
        let installed = locate_sim_source_packages(&manager, &store).unwrap_or_default();
        let mut opened = manager.all_packages_with_ids()?;
        opened.sort_by_key(|(id, _)| *id);
        let dependencies = opened
            .iter()
            .filter(|(id, _)| *id != package_id)
            .map(|(_, package)| package.as_ref())
            .chain(installed.iter().map(|(_, package)| package))
            .collect::<Vec<_>>();
        road_appearance(&selected, tgi.group, tgi.instance, &dependencies)
            .ok_or_else(|| CommandError::internal("road_appearance_invalid"))
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}
