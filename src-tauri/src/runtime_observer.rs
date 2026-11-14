use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::Value;
use tauri::State;

use crate::activity::{AppState, CommandError};

mod bridge_time;

fn observer_python() -> PathBuf {
    #[cfg(windows)]
    if let Some(path) = std::env::var_os("PATH") {
        if let Some(executable) = std::env::split_paths(&path)
            .filter(|path| {
                !path
                    .components()
                    .any(|part| part.as_os_str().eq_ignore_ascii_case("WindowsApps"))
            })
            .map(|path| path.join("python.exe"))
            .find(|path| path.is_file())
        {
            return executable;
        }
    }
    PathBuf::from("python")
}

fn observer_python_command() -> Command {
    let mut command = Command::new(observer_python());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProcess {
    pub pid: u32,
    pub name: String,
    pub memory_kb: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObserverStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub directory: Option<String>,
    pub records: usize,
    pub synced: usize,
    pub status: Option<Value>,
    pub recent: Vec<Value>,
    pub event_counts: HashMap<String, usize>,
    pub game_time_seconds: Option<f64>,
    pub game_clock_seconds: Option<f64>,
    pub game_clock_source: Option<String>,
    pub game_paused: Option<bool>,
    pub clock_advancing: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameFrame {
    pub mime: String,
    pub data_base64: String,
    pub captured_at: u128,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug)]
pub struct ObserverManager {
    child: Mutex<Option<Child>>,
    pid: Mutex<Option<u32>>,
    directory: Mutex<Option<PathBuf>>,
    cursor: Mutex<usize>,
    synced: Mutex<usize>,
    asset_scope: Mutex<bool>,
    stopping: Mutex<bool>,
}

impl ObserverManager {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
            pid: Mutex::new(None),
            directory: Mutex::new(None),
            cursor: Mutex::new(0),
            synced: Mutex::new(0),
            asset_scope: Mutex::new(false),
            stopping: Mutex::new(false),
        }
    }
    fn reap(&self) {
        let mut guard = self.child.lock().unwrap();
        if let Some(child) = guard.as_mut() {
            if child.try_wait().ok().flatten().is_some() {
                *guard = None;
            }
        }
    }
    fn is_running(&self) -> bool {
        self.reap();
        self.child.lock().unwrap().is_some()
    }
}

#[tauri::command]
pub async fn observer_list_processes() -> Result<Vec<GameProcess>, CommandError> {
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(windows)]
        let output = {
            use std::os::windows::process::CommandExt;
            Command::new("tasklist")
                .creation_flags(0x08000000)
                .args(["/FO", "CSV", "/NH"])
                .output()
        }
        .map_err(|e| CommandError::internal(e.to_string()))?;
        #[cfg(not(windows))]
        let output = Command::new("ps")
            .args(["-eo", "pid=,comm=,rss="])
            .output()
            .map_err(|e| CommandError::internal(e.to_string()))?;
        let text = String::from_utf8_lossy(&output.stdout);
        let mut rows = Vec::new();
        for line in text.lines() {
            #[cfg(windows)]
            {
                let parts: Vec<_> = line.split("\",\"").map(|s| s.trim_matches('"')).collect();
                if parts.len() >= 5 {
                    if let Ok(pid) = parts[1].parse() {
                        let mem = parts[4].replace(",", "").replace(" K", "").parse().ok();
                        rows.push(GameProcess {
                            pid,
                            name: parts[0].to_string(),
                            memory_kb: mem,
                        });
                    }
                }
            }
            #[cfg(not(windows))]
            {
                let parts: Vec<_> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(pid) = parts[0].parse() {
                        rows.push(GameProcess {
                            pid,
                            name: parts[1].to_string(),
                            memory_kb: parts.get(2).and_then(|x| x.parse().ok()),
                        });
                    }
                }
            }
        }
        rows.retain(|p| p.name.eq_ignore_ascii_case("SimCity.exe"));
        Ok(rows)
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
}

#[tauri::command]
pub async fn observer_start(
    state: State<'_, AppState>,
    pid: u32,
    seconds: Option<u32>,
    profile: Option<String>,
    observation: Option<String>,
    unit_entry: Option<u32>,
) -> Result<ObserverStatus, CommandError> {
    let manager = state.observer.clone();
    let stopping = manager.stopping.lock().unwrap();
    if *stopping {
        return Err(CommandError::new(
            "observer_busy",
            "Capture cleanup is pending",
        ));
    }
    manager.reap();
    let mut child_guard = manager.child.lock().unwrap();
    if child_guard.is_some() || *manager.asset_scope.lock().unwrap() {
        return Err(CommandError::new("observer_busy", "Capture already active"));
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let dir = Ok::<_, std::io::Error>(root.clone())
        .map_err(|e| CommandError::internal(e.to_string()))?
        .join("tmp")
        .join(format!(
            "openscp-observer-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(dir.parent().unwrap())
        .map_err(|e| CommandError::internal(e.to_string()))?;
    let script = root.join(if unit_entry.is_some() {
        "probes/runtime/asset_watch.py"
    } else {
        "tools/dynamic/ui-observer/native_trace.py"
    });
    let stderr = std::fs::File::create(dir.with_extension("stderr"))
        .map_err(|e| CommandError::internal(e.to_string()))?;
    let mut command = observer_python_command();
    let profile = profile.unwrap_or_else(|| "balanced".into());
    let observation = observation.unwrap_or_else(|| "ui:all".into());
    command.current_dir(&root).args([
        script.to_string_lossy().as_ref(),
        "--pid",
        &pid.to_string(),
        "--seconds",
        &seconds.unwrap_or(60).to_string(),
        "--directory",
        dir.to_string_lossy().as_ref(),
    ]);
    if let Some(unit_entry) = unit_entry {
        command.args(["--unit-entry", &unit_entry.to_string()]);
    } else {
        command.args(["--profile", &profile, "--observation", &observation]);
    }
    let child = command
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|e| CommandError::internal(format!("failed to start observer: {e}")))?;
    *child_guard = Some(child);
    *manager.pid.lock().unwrap() = Some(pid);
    *manager.directory.lock().unwrap() = Some(dir);
    *manager.cursor.lock().unwrap() = 0;
    *manager.synced.lock().unwrap() = 0;
    *manager.asset_scope.lock().unwrap() = unit_entry.is_some();
    Ok(ObserverStatus {
        running: true,
        pid: Some(pid),
        directory: manager
            .directory
            .lock()
            .unwrap()
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        records: 0,
        synced: 0,
        status: None,
        recent: Vec::new(),
        event_counts: HashMap::new(),
        game_time_seconds: None,
        game_clock_seconds: None,
        game_clock_source: None,
        game_paused: None,
        clock_advancing: None,
    })
}

#[tauri::command]
pub async fn observer_stop(
    state: State<'_, AppState>,
    asset_directory: Option<String>,
) -> Result<ObserverStatus, CommandError> {
    {
        let mut stopping = state.observer.stopping.lock().unwrap();
        if *stopping {
            return Err(CommandError::new(
                "observer_busy",
                "Capture cleanup is pending",
            ));
        }
        *stopping = true;
    }
    let result = stop_session(state.clone(), asset_directory).await;
    *state.observer.stopping.lock().unwrap() = false;
    result
}

async fn stop_session(
    state: State<'_, AppState>,
    asset_directory: Option<String>,
) -> Result<ObserverStatus, CommandError> {
    let directory = state.observer.directory.lock().unwrap().clone();
    let asset_scope = *state.observer.asset_scope.lock().unwrap();
    if asset_scope != asset_directory.is_some()
        || asset_directory.as_deref().is_some_and(|expected| {
            directory.as_ref().and_then(|dir| dir.to_str()) != Some(expected)
        })
    {
        return Err(CommandError::new(
            "observer_busy",
            "Another capture owns the session",
        ));
    }
    if let Some(ref dir) = directory {
        std::fs::write(dir.with_extension("stop"), b"stop")
            .map_err(|e| CommandError::internal(e.to_string()))?;
    }
    let manager = state.observer.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(35);
        while manager.is_running() {
            if std::time::Instant::now() >= deadline {
                return Err(CommandError::new(
                    "observer_stop_timeout",
                    "Probe cleanup is still pending",
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Ok(())
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))??;
    if directory.is_some() && !asset_scope {
        observer_sync(state.clone()).await?;
    }
    let snapshot = observer_status(state.clone(), asset_directory).await?;
    if let Some(dir) = directory {
        match std::fs::remove_file(dir.join("trace.jsonl")) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(CommandError::internal(e.to_string())),
        }
    }
    if asset_scope {
        *state.observer.asset_scope.lock().unwrap() = false;
        *state.observer.directory.lock().unwrap() = None;
        *state.observer.pid.lock().unwrap() = None;
    }
    Ok(snapshot)
}

#[tauri::command]
pub async fn observer_status(
    state: State<'_, AppState>,
    asset_directory: Option<String>,
) -> Result<ObserverStatus, CommandError> {
    let manager = state.observer.clone();
    manager.reap();
    let asset_scope = *manager.asset_scope.lock().unwrap();
    let running = manager.is_running();
    let dir = manager.directory.lock().unwrap().clone();
    if asset_scope != asset_directory.is_some()
        || asset_directory
            .as_deref()
            .is_some_and(|expected| dir.as_ref().and_then(|dir| dir.to_str()) != Some(expected))
    {
        return Ok(ObserverStatus {
            running,
            pid: None,
            directory: None,
            records: 0,
            synced: 0,
            status: running.then(|| serde_json::json!({"error": "observer_busy"})),
            recent: Vec::new(),
            event_counts: HashMap::new(),
            game_time_seconds: None,
            game_clock_seconds: None,
            game_clock_source: None,
            game_paused: None,
            clock_advancing: None,
        });
    }
    let status_path = dir.as_ref().map(|p| p.join("status.json"));
    let mut status: Option<Value> = status_path
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok());
    if !running && status.is_none() {
        if let Some(ref dir) = dir {
            let error = std::fs::read_to_string(dir.with_extension("stderr")).unwrap_or_default();
            status = Some(
                serde_json::json!({"error": if error.is_empty() { "Observer exited before ready".to_string() } else { error }}),
            );
        }
    }
    let lines = dir
        .as_ref()
        .map(|p| p.join("trace.jsonl"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| s.lines().map(str::to_owned).collect::<Vec<_>>())
        .unwrap_or_default();
    let mut recent = std::collections::VecDeque::new();
    let mut event_counts = HashMap::new();
    let mut game_time_seconds = None;
    let mut game_clock_seconds = status
        .as_ref()
        .and_then(|status| status.get("gameClockSeconds"))
        .and_then(Value::as_f64);
    let mut game_clock_source = game_clock_seconds.map(|_| "eco_game.day_period".into());
    let mut bridge = bridge_time::BridgeTime::default();
    let mut request_by_call = HashMap::new();
    for line in &lines {
        let Ok(row) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !asset_scope {
            if row.get("name").and_then(Value::as_str) == Some("ReceiveGameEventFromClient") {
                if let Some(id) = row.get("eventID").and_then(Value::as_i64) {
                    *event_counts.entry(id.to_string()).or_insert(0) += 1;
                }
            }
            bridge.observe(&row, &mut request_by_call);
        }
        if recent.len() == 1000 {
            recent.pop_front();
        }
        if row.get("channel").and_then(Value::as_str) == Some("result") {
            let request = row
                .get("requestName")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let value = row.get("result").and_then(|result| result.get("value"));
            if request == "totalElapsedGameTime" {
                game_time_seconds = value.and_then(Value::as_f64);
            }
        }
        recent.push_back(row);
    }
    bridge.finish();
    if let Some(seconds) = bridge.latest_clock() {
        game_clock_seconds = Some(seconds);
        game_clock_source = Some("ui_bridge.periodTimes/0".into());
    }
    let game_paused = bridge.paused();
    let clock_advancing = bridge.advancing();
    for row in &mut recent {
        bridge.annotate(row);
    }
    let records = lines.len();
    Ok(ObserverStatus {
        running,
        pid: *manager.pid.lock().unwrap(),
        directory: dir.map(|p| p.to_string_lossy().to_string()),
        records,
        synced: *manager.synced.lock().unwrap(),
        status,
        recent: recent.into_iter().rev().collect(),
        event_counts,
        game_time_seconds,
        game_clock_seconds,
        game_clock_source,
        game_paused,
        clock_advancing,
    })
}

#[tauri::command]
pub async fn observer_sync(state: State<'_, AppState>) -> Result<usize, CommandError> {
    let dir = state
        .observer
        .directory
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| CommandError::new("observer_not_started", "No observer session"))?;
    let path = dir.join("trace.jsonl");
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(CommandError::internal(e.to_string())),
    };
    let mut cursor = state.observer.cursor.lock().unwrap();
    let mut synced = state.observer.synced.lock().unwrap();
    let mut count = 0;
    for line in text
        .split_inclusive('\n')
        .filter(|line| line.ends_with('\n'))
        .skip(*cursor)
    {
        let payload: Value = serde_json::from_str(line).unwrap_or(Value::String(line.into()));
        let is_event =
            payload.get("name").and_then(Value::as_str) == Some("ReceiveGameEventFromClient");
        if !is_event {
            *cursor += 1;
            continue;
        }
        let event_id = payload
            .get("eventID")
            .and_then(Value::as_i64)
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".into());
        state
            .store
            .append_event(&sc_store::EventInput {
                operation_id: None,
                level: "info".into(),
                topic: "runtime.key_event".into(),
                message: format!("Game event {event_id}"),
                payload: Some(payload),
            })
            .map_err(CommandError::from)?;
        count += 1;
        *synced += 1;
        *cursor += 1;
    }
    Ok(count)
}

/// Read-only game-window capture. The helper uses Win32 PrintWindow and never
/// injects into or hooks the game process.
#[tauri::command]
pub async fn observer_frame(pid: u32) -> Result<GameFrame, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        use base64::Engine;
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let output = root.join("tmp").join(format!(
            "openscp-frame-{}-{}.png",
            pid,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let script = root.join("tools/runtime_capture/window_frame.py");
        let result = observer_python_command()
            .args([
                script.to_string_lossy().as_ref(),
                "--pid",
                &pid.to_string(),
                "--output",
                output.to_string_lossy().as_ref(),
            ])
            .output()
            .map_err(|e| CommandError::new("frame_capture_failed", e.to_string()))?;
        if !result.status.success() {
            let detail = String::from_utf8_lossy(&result.stderr).trim().to_owned();
            let _ = fs::remove_file(&output);
            return Err(CommandError::new(
                "frame_capture_failed",
                if detail.is_empty() {
                    "window capture failed".into()
                } else {
                    detail
                },
            ));
        }
        let bytes = fs::read(&output)
            .map_err(|e| CommandError::new("frame_capture_failed", e.to_string()))?;
        let _ = fs::remove_file(&output);
        if bytes.is_empty() {
            return Err(CommandError::new(
                "frame_capture_failed",
                "captured frame is empty",
            ));
        }
        Ok(GameFrame {
            mime: "image/png".into(),
            data_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            captured_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            width: None,
            height: None,
        })
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
}
