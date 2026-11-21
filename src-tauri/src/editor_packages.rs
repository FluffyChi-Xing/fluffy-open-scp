use crate::{
    activity::{AppState, CommandError},
    package_service::PackageManager,
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tauri::State;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPackagesRequest {
    directory: String,
    kind: EditorPackageKind,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum EditorPackageKind {
    Assets,
    Terrain,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorPackage {
    package_id: u64,
    path: String,
}
#[derive(Serialize)]
pub struct EditorPackagesResult {
    packages: Vec<EditorPackage>,
    missing: Vec<String>,
}

fn acquire(
    manager: &PackageManager,
    directory: &str,
    kind: EditorPackageKind,
) -> EditorPackagesResult {
    let names: &[&str] = match kind {
        EditorPackageKind::Assets => &[
            "SimCity_App.package",
            "SimCity_Game.package",
            "SimCityDataEP1.package",
            "SimCity_Graphics.package",
            "SimCity_DLC0.package",
        ],
        EditorPackageKind::Terrain => &[
            "SimCity_RegionTerrain0.package",
            "SimCity_RegionTerrain1.package",
        ],
    };
    let mut result = EditorPackagesResult {
        packages: vec![],
        missing: vec![],
    };
    for name in names {
        match manager.acquire_editor_package(&PathBuf::from(directory).join(name)) {
            Ok((package_id, package)) => result.packages.push(EditorPackage {
                package_id,
                path: package.path().to_string_lossy().into_owned(),
            }),
            Err(_) => result.missing.push((*name).into()),
        }
    }
    result
}

#[tauri::command]
pub async fn acquire_editor_packages(
    state: State<'_, AppState>,
    request: EditorPackagesRequest,
) -> Result<EditorPackagesResult, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || {
        acquire(&manager, &request.directory, request.kind)
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))
}

#[tauri::command]
pub async fn release_editor_packages(
    state: State<'_, AppState>,
    package_ids: Vec<u64>,
) -> Result<(), CommandError> {
    for id in package_ids {
        state
            .packages
            .release_editor_package(id)
            .map_err(CommandError::from)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
    fn fixture() -> PathBuf {
        let root = crate::test_scratch::scratch_base().join(format!(
            "editor-packages-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let bytes = write_uncompressed_overlay(&[OverlayEntry::new(
            ResourceId {
                type_id: 1,
                group: 2,
                instance: 3,
            },
            b"resource",
        )])
        .unwrap();
        std::fs::write(root.join("SimCity_Game.package"), bytes).unwrap();
        root
    }
    #[test]
    fn editor_packages_keep_explorer_and_other_pages_alive() {
        let root = fixture();
        let path = root.join("SimCity_Game.package");
        let manager = PackageManager::new();
        let first = acquire(&manager, root.to_str().unwrap(), EditorPackageKind::Assets);
        assert_eq!(first.packages.len(), 1);
        assert_eq!(first.missing.len(), 4);
        let editor = first.packages[0].package_id;
        let (explorer, _) = manager.insert(Package::open(&path).unwrap()).unwrap();
        let (second, _) = manager.acquire_editor_package(&path).unwrap();
        assert_ne!(editor, explorer);
        assert_ne!(editor, second);
        manager.release_editor_package(editor).unwrap();
        assert!(manager.get(editor).is_err());
        assert!(manager.get(explorer).is_ok());
        assert!(manager.get(second).is_ok());
        manager.close(explorer).unwrap();
        assert!(manager.get(second).is_ok());
        manager.release_editor_package(explorer).unwrap();
        manager.release_editor_package(second).unwrap();
        assert!(manager.all_packages_with_ids().unwrap().is_empty());
    }
    #[test]
    fn editor_packages_missing_directory_is_non_blocking() {
        let manager = PackageManager::new();
        let result = acquire(
            &manager,
            "nonexistent-directory-for-editor-test",
            EditorPackageKind::Terrain,
        );
        assert!(result.packages.is_empty());
        assert_eq!(
            result.missing,
            [
                "SimCity_RegionTerrain0.package",
                "SimCity_RegionTerrain1.package"
            ]
        );
    }
}
#[tauri::command]
pub async fn acquire_editor_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<EditorPackage, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || {
        let (package_id, package) = manager.acquire_editor_package(&PathBuf::from(path))?;
        Ok::<_, crate::package_service::PackageError>(EditorPackage {
            package_id,
            path: package.path().to_string_lossy().into_owned(),
        })
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
    .map_err(CommandError::from)
}
