use std::{collections::HashMap, fs, path::PathBuf, process::{Child, Command, Stdio}, sync::Mutex, time::{SystemTime, UNIX_EPOCH}};

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::activity::{AppState, CommandError};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProcess { pub pid: u32, pub name: String, pub memory_kb: Option<u64> }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObserverStatus { pub running: bool, pub pid: Option<u32>, pub directory: Option<String>, pub records: usize, pub synced: usize, pub status: Option<Value>, pub recent: Vec<Value>, pub event_counts: HashMap<String, usize>, pub game_time_seconds: Option<f64>, pub game_clock_seconds: Option<f64>, pub game_clock_source: Option<String>, pub game_paused: Option<bool> }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameFrame { pub mime: String, pub data_base64: String, pub captured_at: u128, pub width: Option<u32>, pub height: Option<u32> }

#[derive(Debug)]
pub struct ObserverManager { child: Mutex<Option<Child>>, pid: Mutex<Option<u32>>, directory: Mutex<Option<PathBuf>>, cursor: Mutex<usize>, synced: Mutex<usize> }

impl ObserverManager {
    pub fn new() -> Self { Self { child: Mutex::new(None), pid: Mutex::new(None), directory: Mutex::new(None), cursor: Mutex::new(0), synced: Mutex::new(0) } }
    fn reap(&self) { let mut guard = self.child.lock().unwrap(); if let Some(child) = guard.as_mut() { if child.try_wait().ok().flatten().is_some() { *guard = None; } } }
    fn is_running(&self) -> bool { self.reap(); self.child.lock().unwrap().is_some() }
}

#[tauri::command]
pub async fn observer_list_processes() -> Result<Vec<GameProcess>, CommandError> {
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(windows)]
        let output = { use std::os::windows::process::CommandExt; Command::new("tasklist").creation_flags(0x08000000).args(["/FO", "CSV", "/NH"]).output() }.map_err(|e| CommandError::internal(e.to_string()))?;
        #[cfg(not(windows))]
        let output = Command::new("ps").args(["-eo", "pid=,comm=,rss="]).output().map_err(|e| CommandError::internal(e.to_string()))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut rows = Vec::new();
        for line in text.lines() {
            #[cfg(windows)]
            { let parts: Vec<_> = line.split("\",\"").map(|s| s.trim_matches('"')).collect(); if parts.len() >= 5 { if let Ok(pid) = parts[1].parse() { let mem = parts[4].replace(",", "").replace(" K", "").parse().ok(); rows.push(GameProcess { pid, name: parts[0].to_string(), memory_kb: mem }); } } }
            #[cfg(not(windows))]
            { let parts: Vec<_> = line.split_whitespace().collect(); if parts.len() >= 2 { if let Ok(pid) = parts[0].parse() { rows.push(GameProcess { pid, name: parts[1].to_string(), memory_kb: parts.get(2).and_then(|x| x.parse().ok()) }); } } }
        }
        rows.retain(|p| p.name.eq_ignore_ascii_case("SimCity.exe"));
        Ok(rows)
    }).await.map_err(|e| CommandError::internal(e.to_string()))?
}

#[tauri::command]
pub async fn observer_start(state: State<'_, AppState>, pid: u32, seconds: Option<u32>, profile: Option<String>, observation: Option<String>) -> Result<ObserverStatus, CommandError> {
    let manager = state.observer.clone();
    manager.reap();
    let mut child_guard = manager.child.lock().unwrap();
    if child_guard.is_some() { return Err(CommandError::new("observer_busy", "Capture already active")); }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let dir = Ok::<_, std::io::Error>(root.clone()).map_err(|e| CommandError::internal(e.to_string()))?.join("tmp").join(format!("openscp-observer-{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()));
    std::fs::create_dir_all(dir.parent().unwrap()).map_err(|e| CommandError::internal(e.to_string()))?;
    let script = root.join("tools/dynamic/ui-observer/native_trace.py");
    let stderr = std::fs::File::create(dir.with_extension("stderr")).map_err(|e| CommandError::internal(e.to_string()))?;
    let mut command = Command::new("python");
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let profile = profile.unwrap_or_else(|| "balanced".into());
    let observation = observation.unwrap_or_else(|| "ui:all".into());
    let child = command.current_dir(&root).args([script.to_string_lossy().as_ref(), "--pid", &pid.to_string(), "--seconds", &seconds.unwrap_or(60).to_string(), "--profile", &profile, "--observation", &observation, "--directory", dir.to_string_lossy().as_ref()]).stdout(Stdio::null()).stderr(Stdio::from(stderr)).spawn().map_err(|e| CommandError::internal(format!("failed to start observer: {e}")))?;
    *child_guard = Some(child); *manager.pid.lock().unwrap() = Some(pid); *manager.directory.lock().unwrap() = Some(dir); *manager.cursor.lock().unwrap() = 0; *manager.synced.lock().unwrap() = 0;
    Ok(ObserverStatus { running: true, pid: Some(pid), directory: manager.directory.lock().unwrap().as_ref().map(|p| p.to_string_lossy().to_string()), records: 0, synced: 0, status: None, recent: Vec::new(), event_counts: HashMap::new(), game_time_seconds: None, game_clock_seconds: None, game_clock_source: None, game_paused: None })
}

#[tauri::command]
pub async fn observer_stop(state: State<'_, AppState>) -> Result<ObserverStatus, CommandError> {
    if let Some(dir) = state.observer.directory.lock().unwrap().clone() {
        std::fs::write(dir.with_extension("stop"), b"stop")
            .map_err(|e| CommandError::internal(e.to_string()))?;
    }
    let manager = state.observer.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(35);
        while manager.is_running() {
            if std::time::Instant::now() >= deadline {
                return Err(CommandError::new("observer_stop_timeout", "Probe cleanup is still pending"));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Ok(())
    }).await.map_err(|e| CommandError::internal(e.to_string()))??;
    if state.observer.directory.lock().unwrap().is_some() {
        observer_sync(state.clone()).await?;
    }
    let snapshot = observer_status(state.clone()).await?;
    if let Some(dir) = state.observer.directory.lock().unwrap().clone() {
        match std::fs::remove_file(dir.join("trace.jsonl")) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(CommandError::internal(e.to_string())),
        }
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn observer_status(state: State<'_, AppState>) -> Result<ObserverStatus, CommandError> {
    let manager = state.observer.clone(); manager.reap(); let running = manager.is_running(); let dir = manager.directory.lock().unwrap().clone();
    let status_path = dir.as_ref().map(|p| p.join("status.json")); let mut status: Option<Value> = status_path.and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok());
    if !running && status.is_none() {
        if let Some(ref dir) = dir {
            let error = std::fs::read_to_string(dir.with_extension("stderr")).unwrap_or_default();
            status = Some(serde_json::json!({"error": if error.is_empty() { "Observer exited before ready".to_string() } else { error }}));
        }
    }
    let lines = dir.as_ref().map(|p| p.join("trace.jsonl")).and_then(|p| std::fs::read_to_string(p).ok()).map(|s| s.lines().map(str::to_owned).collect::<Vec<_>>()).unwrap_or_default();
    let recent = lines.iter().rev().take(80).filter_map(|line| serde_json::from_str::<Value>(line).ok()).collect::<Vec<_>>();
    let mut event_counts = HashMap::new(); for line in &lines { if let Ok(row) = serde_json::from_str::<Value>(line) { if row.get("name").and_then(Value::as_str) == Some("ReceiveGameEventFromClient") { if let Some(id) = row.get("eventID").and_then(Value::as_i64) { *event_counts.entry(id.to_string()).or_insert(0) += 1; } } } }
    let mut game_time_seconds = None;
    let mut game_clock_seconds = None;
    let mut game_clock_source = None;
    let mut game_paused = None;
    let mut request_by_call: HashMap<i64, String> = HashMap::new();
    for line in &lines {
        let Ok(row) = serde_json::from_str::<Value>(line) else { continue };
        if row.get("channel").and_then(Value::as_str) == Some("invoke") {
            if let (Some(sequence), Some(text)) = (
                row.get("sequence").and_then(Value::as_i64),
                row.get("arguments").and_then(Value::as_array)
                    .and_then(|args| args.first())
                    .and_then(|arg| arg.get("text"))
                    .and_then(Value::as_str),
            ) {
                request_by_call.insert(sequence, text.to_string());
            }
            continue;
        }
        if row.get("channel").and_then(Value::as_str) != Some("result") { continue; }
        let request = row.get("requestName").and_then(Value::as_str).unwrap_or_default();
        let value = row.get("result").and_then(|result| result.get("value"));
        if request == "totalElapsedGameTime" { game_time_seconds = value.and_then(Value::as_f64); }
        if request == "periodTimes" {
            let call_sequence = row.get("callSequence").and_then(Value::as_i64);
            let full_request = call_sequence.and_then(|seq| request_by_call.get(&seq).map(String::as_str));
            if full_request == Some("periodTimes/0") {
                if let Some(fraction) = value.and_then(Value::as_f64).filter(|v| v.is_finite()) {
                    game_clock_seconds = Some(fraction.rem_euclid(1.0) * 86_400.0);
                    game_clock_source = Some("ui_bridge.periodTimes/0".into());
                }
            }
        }
        if request.starts_with("globalFlags") { game_paused = value.and_then(Value::as_bool); }
    }
    let records = lines.len();
    Ok(ObserverStatus { running, pid: *manager.pid.lock().unwrap(), directory: dir.map(|p| p.to_string_lossy().to_string()), records, synced: *manager.synced.lock().unwrap(), status, recent, event_counts, game_time_seconds, game_clock_seconds, game_clock_source, game_paused })
}

#[tauri::command]
pub async fn observer_sync(state: State<'_, AppState>) -> Result<usize, CommandError> {
    let dir = state.observer.directory.lock().unwrap().clone().ok_or_else(|| CommandError::new("observer_not_started", "No observer session"))?;
    let path = dir.join("trace.jsonl"); let text = match std::fs::read_to_string(path) { Ok(text) => text, Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0), Err(e) => return Err(CommandError::internal(e.to_string())) }; let mut cursor = state.observer.cursor.lock().unwrap(); let mut synced = state.observer.synced.lock().unwrap(); let mut count = 0;
    for line in text.split_inclusive('\n').filter(|line| line.ends_with('\n')).skip(*cursor) {
        let payload: Value = serde_json::from_str(line).unwrap_or(Value::String(line.into()));
        let is_event = payload.get("name").and_then(Value::as_str) == Some("ReceiveGameEventFromClient");
        if !is_event { *cursor += 1; continue; }
        let event_id = payload.get("eventID").and_then(Value::as_i64).map(|id| id.to_string()).unwrap_or_else(|| "unknown".into());
        state.store.append_event(&sc_store::EventInput { operation_id: None, level: "info".into(), topic: "runtime.key_event".into(), message: format!("Game event {event_id}"), payload: Some(payload) }).map_err(CommandError::from)?; count += 1; *synced += 1; *cursor += 1;
    }
    Ok(count)
}

/// Read-only game-window capture. The helper uses Win32 PrintWindow and never
/// injects into or hooks the game process.
#[tauri::command]
pub async fn observer_frame(pid: u32) -> Result<GameFrame, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        use base64::Engine;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
        let output = root.join("tmp").join(format!("openscp-frame-{}-{}.png", pid, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        let script = root.join("tools/runtime_capture/window_frame.py");
        let result = Command::new("python").args([script.to_string_lossy().as_ref(), "--pid", &pid.to_string(), "--output", output.to_string_lossy().as_ref()]).output()
            .map_err(|e| CommandError::new("frame_capture_failed", e.to_string()))?;
        if !result.status.success() {
            let detail = String::from_utf8_lossy(&result.stderr).trim().to_owned();
            let _ = fs::remove_file(&output);
            return Err(CommandError::new("frame_capture_failed", if detail.is_empty() { "window capture failed".into() } else { detail }));
        }
        let bytes = fs::read(&output).map_err(|e| CommandError::new("frame_capture_failed", e.to_string()))?;
        let _ = fs::remove_file(&output);
        if bytes.is_empty() { return Err(CommandError::new("frame_capture_failed", "captured frame is empty")); }
        Ok(GameFrame { mime: "image/png".into(), data_base64: base64::engine::general_purpose::STANDARD.encode(bytes), captured_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(), width: None, height: None })
    }).await.map_err(|e| CommandError::internal(e.to_string()))?
}
