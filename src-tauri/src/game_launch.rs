use std::{fs, path::{Path, PathBuf}, process::Command};

use serde::Serialize;
use tauri::State;

use crate::activity::{AppState, CommandError};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProcessInfo { pub pid: u32, pub name: String }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployGameModResult {
    pub artifact: String,
    pub destination: String,
    pub game_running: bool,
    pub processes: Vec<GameProcessInfo>,
}

fn processes() -> Result<Vec<GameProcessInfo>, CommandError> {
    #[cfg(windows)]
    let output = Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
        .map_err(|e| CommandError::internal(e.to_string()))?;
    #[cfg(not(windows))]
    let output = Command::new("ps")
        .args(["-eo", "pid=,comm="])
        .output()
        .map_err(|e| CommandError::internal(e.to_string()))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut result = Vec::new();
    for line in text.lines() {
        #[cfg(windows)]
        let fields: Vec<String> = line.split("\",\"").map(|s| s.trim_matches('\"').to_string()).collect();
        #[cfg(not(windows))]
        let fields: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        if fields.len() >= 2 {
            let (name, pid) = {
                #[cfg(windows)] { (fields[0].clone(), fields[1].parse::<u32>().ok()) }
                #[cfg(not(windows))] { (fields[1].clone(), fields[0].parse::<u32>().ok()) }
            };
            if name.eq_ignore_ascii_case("SimCity.exe") {
                if let Some(pid) = pid { result.push(GameProcessInfo { pid, name }); }
            }
        }
    }
    Ok(result)
}

fn safe_filename(name: &str) -> String {
    let mut value: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if value.is_empty() { value = "openscp-mod".into(); }
    format!("OpenSCP_{value}.package")
}

fn copy_atomic(source: &Path, destination: &Path) -> Result<(), CommandError> {
    let temp = destination.with_extension("package.tmp");
    fs::copy(source, &temp).map_err(|e| CommandError::new("deploy_failed", e.to_string()))?;
    if destination.exists() {
        fs::remove_file(destination).map_err(|e| {
            let _ = fs::remove_file(&temp);
            CommandError::new("deploy_failed", e.to_string())
        })?;
    }
    if let Err(error) = fs::rename(&temp, destination) {
        let _ = fs::remove_file(&temp);
        return Err(CommandError::new("deploy_failed", error.to_string()));
    }
    Ok(())
}

#[tauri::command]
pub async fn game_processes() -> Result<Vec<GameProcessInfo>, CommandError> {
    tauri::async_runtime::spawn_blocking(processes)
        .await.map_err(|e| CommandError::internal(e.to_string()))?
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployGameModRequest { pub artifact: String, pub project: String }

#[tauri::command]
pub async fn deploy_game_mod(state: State<'_, AppState>, request: DeployGameModRequest) -> Result<DeployGameModResult, CommandError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let artifact = PathBuf::from(&request.artifact);
        if !artifact.is_file() || artifact.extension().and_then(|v| v.to_str()) != Some("package") {
            return Err(CommandError::new("deploy_invalid_artifact", "Build artifact is not a .package file"));
        }
        let game_data = store.app_settings().map_err(CommandError::from)?.and_then(|s| s.game_data_path)
            .ok_or_else(|| CommandError::new("game_directory_not_configured", "Game data directory is not configured"))?;
        let game_data = PathBuf::from(game_data);
        if !game_data.is_dir() { return Err(CommandError::new("game_directory_unavailable", "Game data directory is unavailable")); }
        let packages = game_data.parent().unwrap_or(&game_data).join("SimCityUserData").join("Packages");
        fs::create_dir_all(&packages).map_err(|e| CommandError::new("deploy_failed", e.to_string()))?;
        let destination = packages.join(safe_filename(&request.project));
        copy_atomic(&artifact, &destination)?;
        let found = processes()?;
        Ok(DeployGameModResult { artifact: artifact.to_string_lossy().into_owned(), destination: destination.to_string_lossy().into_owned(), game_running: !found.is_empty(), processes: found })
    }).await.map_err(|e| CommandError::internal(e.to_string()))?
}

#[tauri::command]
pub async fn launch_game(state: State<'_, AppState>) -> Result<GameProcessInfo, CommandError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(existing) = processes()?.into_iter().next() { return Ok(existing); }
        let game_data = store.app_settings().map_err(CommandError::from)?.and_then(|s| s.game_data_path)
            .ok_or_else(|| CommandError::new("game_directory_not_configured", "Game data directory is not configured"))?;
        let root = PathBuf::from(game_data).parent().map(Path::to_path_buf)
            .ok_or_else(|| CommandError::new("game_directory_unavailable", "Cannot determine game root"))?;
        let exe = [root.join("SimCity.exe"), root.join("SimCity").join("SimCity.exe"), root.join("SimCityUserData").join("Patches").join("UpdatedApp").join("SimCity.exe")]
            .into_iter().find(|candidate| candidate.is_file())
            .unwrap_or_else(|| root.join("SimCity.exe"));
        if !exe.is_file() { return Err(CommandError::new("game_executable_missing", format!("SimCity.exe not found at {}", exe.display()))); }
        Command::new(&exe).current_dir(exe.parent().unwrap_or(&root)).spawn().map_err(|e| CommandError::new("game_launch_failed", e.to_string()))?;
        for _ in 0..20 { std::thread::sleep(std::time::Duration::from_millis(250)); if let Some(found) = processes()?.into_iter().next() { return Ok(found); } }
        Err(CommandError::new("game_launch_timeout", "Game process did not appear after launch"))
    }).await.map_err(|e| CommandError::internal(e.to_string()))?
}
