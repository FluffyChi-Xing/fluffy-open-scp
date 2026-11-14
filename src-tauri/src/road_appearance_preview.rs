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

#[tauri::command]
pub(crate) async fn read_road_recipe_appearance(
    state: State<'_, AppState>,
    project: String,
    game_package: String,
    recipe: crate::road_asset::Recipe,
    future: bool,
    streetlights: crate::road_asset::Streetlights,
) -> Result<(u32, RoadAssets), CommandError> {
    use crate::road_asset::{Recipe, Streetlights};
    use dbpf::{Package, ResourceId};
    use sc_properties::{PropertyFile, Value};
    const PROP: u32 = 0x00B1_B104;

    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let root = crate::workspace::code_root(&store, &project)?;
        let game =
            crate::road_asset::open_source(&root, &game_package).map_err(CommandError::internal)?;
        let missing = || CommandError::internal("road_recipe_appearance_missing");
        let (_, instance) = recipe.donor();
        let path_id = ResourceId {
            type_id: PROP,
            group: 0x40E1_C400,
            instance,
        };
        let bytes = game
            .read(game.entry(path_id).ok_or_else(missing)?)
            .map_err(|error| CommandError::internal(error.to_string()))?;
        let path = PropertyFile::parse(&bytes)
            .map_err(|error| CommandError::internal(error.to_string()))?;
        // Ground appearances: the futuristic column uses the separate CoT
        // material and excludes the below-grade wall rows in 0x10248904.
        let column = if future && !matches!(recipe, Recipe::Dirt) {
            0x107E_2B61
        } else {
            0x0953_2375
        };
        let Some(Value::Key(key)) = path.get(column).and_then(|property| property.scalar()) else {
            return Err(missing());
        };
        let siblings = [
            "SimCity_Graphics.package",
            "SimCity_App.package",
            "SimCityDataEP1.package",
            "SimCity_DLC0.package",
        ]
        .into_iter()
        .filter_map(|name| Package::open(game.path().with_file_name(name)).ok())
        .collect::<Vec<_>>();
        let dependencies = siblings.iter().collect::<Vec<_>>();
        let mut assets =
            road_appearance(&game, key.group, key.instance, &dependencies).ok_or_else(missing)?;
        // Develop shows an at-grade sample. The resource inspector still
        // exposes the entire extrusion, including below-grade wall panels.
        if let Some(ribbons) = assets.ribbons.get_mut(&key.instance) {
            ribbons.retain(|part| part.offset[2] >= -1.0);
        }
        if let Some(sweeps) = assets.sweeps.get_mut(&key.instance) {
            sweeps.retain(|part| {
                if part.offset[2] < -1.0 {
                    return false;
                }
                if future || !matches!(recipe, Recipe::HighStreet) || part.model != 0x4542_BF83 {
                    return true;
                }
                match streetlights {
                    Streetlights::Original => true,
                    Streetlights::Left => part.offset[0] < 0.0,
                    Streetlights::Right => part.offset[0] > 0.0,
                    Streetlights::None => false,
                }
            });
        }
        Ok((key.instance, assets))
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}
