use crate::activity::CommandError;
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::Duration,
};
use tauri::{Manager, State};
#[derive(Default)]
pub struct CaptureState(Mutex<Option<Child>>);
impl Drop for CaptureState {
    fn drop(&mut self) {
        if let Ok(child) = self.0.get_mut() {
            if let Some(c) = child {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}
fn error(e: impl std::fmt::Display) -> CommandError {
    CommandError::new("devtool_capture", e.to_string())
}
pub fn root(app: &tauri::AppHandle) -> Result<PathBuf, CommandError> {
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(error)?
        .join("devtool"))
}
pub fn install(root: &Path) -> Result<(), CommandError> {
    let service = root.join("service");
    fs::create_dir_all(&service).map_err(error)?;
    for (name, source) in [
        ("service.py", include_str!("capture/service.py")),
        ("receiver.py", include_str!("capture/receiver.py")),
        ("peer.py", include_str!("capture/peer.py")),
        ("overlay_feed.py", include_str!("capture/overlay_feed.py")),
        (
            "validate_bounds.py",
            include_str!("capture/validate_bounds.py"),
        ),
        ("partial_frame.py", include_str!("capture/partial_frame.py")),
        ("archive_store.py", include_str!("capture/archive_store.py")),
        ("auto_index.py", include_str!("capture/auto_index.py")),
    ] {
        fs::write(service.join(name), source).map_err(error)?;
    }
    fs::create_dir_all(root.join("live")).map_err(error)?;
    Ok(())
}
fn python() -> PathBuf {
    if let Some(path) = std::env::var_os("PATH") {
        if let Some(exe) = std::env::split_paths(&path)
            .filter(|p| {
                !p.components()
                    .any(|c| c.as_os_str().eq_ignore_ascii_case("WindowsApps"))
            })
            .map(|p| p.join("python.exe"))
            .find(|p| p.is_file())
        {
            return exe;
        }
    }
    PathBuf::from("python")
}
pub fn start(app: &tauri::AppHandle, state: &CaptureState) -> Result<(), CommandError> {
    let mut guard = state.0.lock().map_err(error)?;
    if let Some(c) = guard.as_mut() {
        if c.try_wait().map_err(error)?.is_none() {
            return Ok(());
        }
    }
    let root = root(app)?;
    if !root.join("capture.json").is_file() {
        return Err(error(
            "Capture configuration missing: import the existing capture profile first",
        ));
    }
    install(&root)?;
    let log = fs::File::create(root.join("receiver-error.log")).map_err(error)?;
    let mut command = Command::new(python());
    command
        .arg("-u")
        .arg(root.join("service/service.py"))
        .arg("--runtime")
        .arg(&root)
        .arg("--parent")
        .arg(std::process::id().to_string())
        .stdout(Stdio::piped())
        .stderr(log);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(error)?;
    let output = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(output).read_line(&mut line).map(|_| line);
        let _ = tx.send(result);
    });
    let ready = rx
        .recv_timeout(Duration::from_secs(8))
        .ok()
        .and_then(Result::ok)
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .is_some_and(|v| v["ready"] == true);
    if !ready {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error(
            fs::read_to_string(root.join("receiver-error.log"))
                .unwrap_or_else(|_| "Capture startup failed or timed out".into()),
        ));
    }
    *guard = Some(child);
    Ok(())
}
pub fn stop(state: &CaptureState) -> Result<(), CommandError> {
    let mut guard = state.0.lock().map_err(error)?;
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}
/// Import an explicit profile directory, never secrets embedded in the binary.
#[tauri::command]
pub fn devtool_import_capture(
    app: tauri::AppHandle,
    state: State<CaptureState>,
    source: String,
) -> Result<(), CommandError> {
    let guard = state.0.lock().map_err(error)?;
    if guard.is_some() {
        return Err(error(
            "Disconnect the inspector before importing capture configuration",
        ));
    }
    let source = PathBuf::from(source);
    let config = fs::read(source.join("capture.json")).map_err(error)?;
    let v: Value = serde_json::from_slice(&config).map_err(error)?;
    if !v["gameExecutable"]
        .as_str()
        .is_some_and(|p| Path::new(p).is_file())
        || !v["allowedPages"].as_array().is_some_and(|a| {
            !a.is_empty()
                && a.iter().all(|p| {
                    p.as_str()
                        .is_some_and(|s| s.starts_with("game://localhost/resource/"))
                })
        })
    {
        return Err(error("Invalid capture profile"));
    }
    let key = fs::read_to_string(source.join("live/session-key")).map_err(error)?;
    if key.trim().len() != 48 || !key.trim().bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(error("Invalid capture credential"));
    }
    let root = root(&app)?;
    install(&root)?;
    fs::write(root.join("live/session-key"), key.trim()).map_err(error)?;
    fs::write(root.join("capture.json"), config).map_err(error)?;
    Ok(())
}
