use crate::activity::CommandError;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
};
use tauri::{Emitter, Manager, State};
pub struct Bridge {
    child: Child,
    pid: u32,
    directory: PathBuf,
    serial: u64,
}
impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[derive(Default)]
pub struct DevtoolState(Mutex<Option<Bridge>>);
fn error(e: impl std::fmt::Display) -> CommandError {
    CommandError::new("devtool_bridge", e.to_string())
}
#[tauri::command]
pub async fn devtool_connect(
    app: tauri::AppHandle,
    pid: u32,
    session: String,
) -> Result<(), CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<DevtoolState>();
        let capture = app.state::<super::runtime::CaptureState>();
        connect_impl(app.clone(), state, capture, pid, session)
    })
    .await
    .map_err(error)?
}
fn connect_impl(
    app: tauri::AppHandle,
    state: State<DevtoolState>,
    capture: State<super::runtime::CaptureState>,
    pid: u32,
    session: String,
) -> Result<(), CommandError> {
    let mut guard = state.0.lock().map_err(error)?;
    *guard = None;
    super::runtime::start(&app, &capture)?;
    let directory = std::env::temp_dir().join(format!(
        "openscp-devtool-{}-{}-{}",
        std::process::id(),
        pid,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(error)?
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).map_err(error)?;
    let script = directory.join("overlay.ps1");
    std::fs::write(&script, include_str!("overlay.ps1")).map_err(error)?;
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script)
        .arg("-GamePid")
        .arg(pid.to_string())
        .arg("-Feed")
        .arg(super::live(&app)?.join(format!("overlay-{pid}.json")))
        .arg("-DesktopPid")
        .arg(std::process::id().to_string())
        .arg("-Requests")
        .arg(&directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().map_err(error)?;
    let output = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let events = app.clone();
    let stderr_session = session.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if let Ok(mut data) = serde_json::from_str::<Value>(&line) {
                data["session"] = json!(session);
                let _ = events.emit("devtool-event", data);
            }
        }
        let _ = events.emit(
            "devtool-event",
            json!({"kind":"closed","pid":pid,"session":session}),
        );
    });
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = app.emit(
                "devtool-event",
                json!({"kind":"error","pid":pid,"session":stderr_session,"message":line}),
            );
        }
    });
    *guard = Some(Bridge {
        child,
        pid,
        directory,
        serial: 0,
    });
    Ok(())
}
#[tauri::command]
pub fn devtool_control(
    state: State<DevtoolState>,
    pid: u32,
    action: String,
    node: Option<u64>,
    sequence: Option<u64>,
    time: Option<u64>,
) -> Result<(), CommandError> {
    if !["highlight", "clear", "pick"].contains(&action.as_str()) {
        return Err(error("Invalid inspector action"));
    }
    let mut guard = state.0.lock().map_err(error)?;
    let b = guard
        .as_mut()
        .filter(|b| b.pid == pid)
        .ok_or_else(|| error("Inspector not connected"))?;
    if b.child.try_wait().map_err(error)?.is_some() {
        return Err(error("Inspector stopped"));
    }
    b.serial += 1;
    let path = b.directory.join(format!("request-{}.json", b.serial));
    let temp = path.with_extension("pending");
    std::fs::write(
        &temp,
        serde_json::to_vec(&json!({"action":action,"node":node,"sequence":sequence,"time":time}))
            .map_err(error)?,
    )
    .map_err(error)?;
    std::fs::rename(temp, path).map_err(error)?;
    Ok(())
}
#[tauri::command]
pub fn devtool_disconnect(
    state: State<DevtoolState>,
    capture: State<super::runtime::CaptureState>,
    pid: u32,
) -> Result<(), CommandError> {
    let mut guard = state.0.lock().map_err(error)?;
    if guard.as_ref().is_some_and(|b| b.pid == pid) {
        *guard = None;
    }
    if guard.is_none() {
        super::runtime::stop(&capture)?;
    }
    Ok(())
}
