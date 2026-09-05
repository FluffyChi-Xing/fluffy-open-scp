use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sc_store::{AppSettings, StoreError};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::activity::{AppState, CommandError};

pub const DEFAULT_GAME_DATA_PATH: &str = r"C:\Games\SimCity\SimCityData";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDirectoryStatus {
    pub path: String,
    pub exists: bool,
    pub is_directory: bool,
    pub accessible: bool,
    pub has_package_marker: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsStatus {
    pub game_data_path: Option<String>,
    pub game_directory: Option<GameDirectoryStatus>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDirectoryRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDirectoryDetection {
    pub candidates: Vec<GameDirectoryStatus>,
}

#[derive(Debug, thiserror::Error)]
enum SettingsError {
    #[error("settings path contains a NUL byte")]
    NulPath,
    #[error("game data path is not a directory")]
    NotDirectory,
    #[error("game data path is a symbolic link or reparse point")]
    Symlink,
    #[error("game data path is not available")]
    NotFound,
    #[error("settings io error: {0}")]
    Io(#[from] io::Error),
    #[error("settings store error: {0}")]
    Store(#[from] StoreError),
}

impl SettingsError {
    fn code(&self) -> &'static str {
        match self {
            Self::NulPath => "invalid_path",
            Self::NotDirectory => "invalid_type",
            Self::Symlink => "path_forbidden",
            Self::NotFound => "not_found",
            Self::Io(_) | Self::Store(_) => "settings_error",
        }
    }
}

impl From<SettingsError> for CommandError {
    fn from(error: SettingsError) -> Self {
        CommandError::new(error.code(), error.to_string())
    }
}

fn inspect_directory(path: &Path) -> GameDirectoryStatus {
    let path_string = path.to_string_lossy().into_owned();
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return GameDirectoryStatus {
            path: path_string,
            exists: false,
            is_directory: false,
            accessible: false,
            has_package_marker: false,
        };
    };
    if metadata.file_type().is_symlink() {
        return GameDirectoryStatus {
            path: path_string,
            exists: true,
            is_directory: false,
            accessible: false,
            has_package_marker: false,
        };
    }
    let is_directory = metadata.is_dir();
    let accessible = is_directory && fs::read_dir(path).is_ok();
    let has_package_marker = accessible && has_package_file(path);
    GameDirectoryStatus {
        path: path_string,
        exists: true,
        is_directory,
        accessible,
        has_package_marker,
    }
}

fn has_package_file(path: &Path) -> bool {
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| {
            let metadata = fs::symlink_metadata(entry.path()).ok();
            metadata.is_some_and(|metadata| {
                metadata.is_file()
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("package"))
            })
        })
}

fn validate_game_directory(path: &str) -> Result<PathBuf, SettingsError> {
    if path.is_empty() || path.contains('\0') {
        return Err(SettingsError::NulPath);
    }
    let path = PathBuf::from(path);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            SettingsError::NotFound
        } else {
            SettingsError::Io(error)
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(SettingsError::Symlink);
    }
    if !metadata.is_dir() {
        return Err(SettingsError::NotDirectory);
    }
    Ok(fs::canonicalize(path)?)
}

fn status_from_settings(settings: Option<AppSettings>) -> SettingsStatus {
    let Some(settings) = settings else {
        return SettingsStatus {
            game_data_path: None,
            game_directory: None,
        };
    };
    let Some(game_data_path) = settings.game_data_path else {
        return SettingsStatus {
            game_data_path: None,
            game_directory: None,
        };
    };
    let path = PathBuf::from(&game_data_path);
    SettingsStatus {
        game_data_path: Some(game_data_path),
        game_directory: Some(inspect_directory(&path)),
    }
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<SettingsStatus, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        store
            .app_settings()
            .map(status_from_settings)
            .map_err(CommandError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn settings_set_game_directory(
    state: State<'_, AppState>,
    request: GameDirectoryRequest,
) -> Result<SettingsStatus, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let canonical = validate_game_directory(&request.path).map_err(CommandError::from)?;
        let settings = store
            .set_game_data_path(Some(&canonical.to_string_lossy()))
            .map_err(CommandError::from)?;
        Ok(status_from_settings(Some(settings)))
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn game_directory_detect(
    state: State<'_, AppState>,
) -> Result<GameDirectoryDetection, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let configured = store
            .app_settings()
            .map_err(CommandError::from)?
            .and_then(|settings| settings.game_data_path.map(PathBuf::from));
        let default = PathBuf::from(DEFAULT_GAME_DATA_PATH);
        let mut paths = Vec::new();
        if let Some(configured) = configured {
            paths.push(configured);
        }
        if !paths.iter().any(|path| path == &default) {
            paths.push(default);
        }
        Ok(GameDirectoryDetection {
            candidates: paths.iter().map(|path| inspect_directory(path)).collect(),
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn temp_dir(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("openscp-settings-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn detects_direct_package_marker_without_recursive_scan() {
        let root = temp_dir("marker");
        File::create(root.join("SimCity.package")).unwrap();
        let status = inspect_directory(&root);
        assert!(status.exists);
        assert!(status.is_directory);
        assert!(status.accessible);
        assert!(status.has_package_marker);
        let nested = root.join("nested");
        fs::create_dir_all(&nested).unwrap();
        File::create(nested.join("nested.package")).unwrap();
        assert!(!inspect_directory(&temp_dir("empty")).has_package_marker);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_invalid_game_directory_paths() {
        assert!(matches!(
            validate_game_directory(""),
            Err(SettingsError::NulPath)
        ));
        assert!(matches!(
            validate_game_directory("bad\0path"),
            Err(SettingsError::NulPath)
        ));
        assert!(matches!(
            validate_game_directory("missing"),
            Err(SettingsError::NotFound)
        ));
    }
}
