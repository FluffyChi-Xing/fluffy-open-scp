pub mod bridge;
use crate::activity::CommandError;
use serde_json::Value;
use std::path::PathBuf;

pub mod runtime;
fn live(app: &tauri::AppHandle) -> Result<PathBuf, CommandError> {
    Ok(runtime::root(app)?.join("live"))
}

#[tauri::command]
pub async fn devtool_elements(app: tauri::AppHandle, pid: u32) -> Result<Option<Value>, CommandError> {
    let capture_app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        let state = capture_app.state::<runtime::CaptureState>();
        runtime::start(&capture_app, &state)
    })
    .await
    .map_err(|e| CommandError::new("devtool_capture", e.to_string()))??;
    let path = live(&app)?.join(format!("overlay-{pid}.json"));
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read(path).map_err(|e| CommandError::new("devtool_read", e.to_string()))?;
    let data: Value = serde_json::from_slice(&raw)
        .map_err(|e| CommandError::new("devtool_parse", e.to_string()))?;
    if data["pid"].as_u64() != Some(pid as u64) {
        return Err(CommandError::new(
            "devtool_pid",
            "Snapshot process mismatch",
        ));
    }
    Ok(Some(data))
}
