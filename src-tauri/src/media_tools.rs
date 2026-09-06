use std::ffi::{OsStr, OsString};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde::Serialize;

pub const VGMSTREAM_RELATIVE_PATH: &str = "Tools/vgmstream/vgmstream-cli.exe";
pub const FFMPEG_RELATIVE_PATH: &str = "Tools/ffmpeg/ffmpeg.exe";
pub const MEDIA_PROCESS_LIMIT: usize = 2;
pub const MEDIA_PROCESS_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MAX_STDERR_BYTES: usize = 64 * 1024;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaTool {
    pub available: bool,
    pub path: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaTools {
    pub vgmstream: MediaTool,
    pub ffmpeg: MediaTool,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("failed to start {tool}: {source}")]
    Spawn {
        tool: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("{tool} exited with status {status}: {stderr}")]
    Failed {
        tool: &'static str,
        status: String,
        stderr: String,
    },
    #[error("{tool} timed out after {timeout_ms} ms")]
    Timeout { tool: &'static str, timeout_ms: u64 },
    #[error("failed while waiting for {tool}: {source}")]
    Wait {
        tool: &'static str,
        #[source]
        source: io::Error,
    },
}

pub fn resolve_tools(application_dir: &Path, path: Option<&OsStr>) -> MediaTools {
    let vgmstream_path = application_dir.join(VGMSTREAM_RELATIVE_PATH);
    let vgmstream = tool_info(vgmstream_path, "bundled")
        .or_else(|| {
            path.and_then(|path| {
                std::env::split_paths(path)
                    .filter(|directory| !directory.as_os_str().is_empty())
                    .map(|directory| directory.join(executable_name("vgmstream-cli")))
                    .find_map(|candidate| tool_info(candidate, "path"))
            })
        })
        .unwrap_or_else(unavailable);
    let ffmpeg_path = application_dir.join(FFMPEG_RELATIVE_PATH);
    let ffmpeg = tool_info(ffmpeg_path, "bundled")
        .or_else(|| {
            path.and_then(|path| {
                std::env::split_paths(path)
                    .filter(|directory| !directory.as_os_str().is_empty())
                    .map(|directory| directory.join(executable_name("ffmpeg")))
                    .find_map(|candidate| tool_info(candidate, "path"))
            })
        })
        .unwrap_or_else(unavailable);
    MediaTools { vgmstream, ffmpeg }
}

pub fn run(tool: &'static str, executable: &Path, args: &[OsString]) -> Result<(), ToolError> {
    run_with_timeout(tool, executable, args, MEDIA_PROCESS_TIMEOUT)
}

pub fn run_with_timeout(
    tool: &'static str,
    executable: &Path,
    args: &[OsString],
    timeout: Duration,
) -> Result<(), ToolError> {
    let _permit = MediaPermit::acquire();
    let mut command = Command::new(executable);
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| ToolError::Spawn { tool, source })?;
    let stderr = child.stderr.take().ok_or_else(|| ToolError::Spawn {
        tool,
        source: io::Error::new(io::ErrorKind::Other, "stderr pipe was not created"),
    })?;
    let reader = thread::spawn(move || {
        let mut stderr = stderr;
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            match stderr.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    let remaining = MAX_STDERR_BYTES.saturating_sub(bytes.len());
                    bytes.extend_from_slice(&buffer[..count.min(remaining)]);
                }
            }
        }
        bytes
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(ToolError::Timeout {
                    tool,
                    timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
                });
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(source) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err(ToolError::Wait { tool, source });
            }
        }
    };
    let stderr = reader.join().unwrap_or_default();
    if status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&stderr);
        let stderr = stderr.lines().take(4).collect::<Vec<_>>().join(" ");
        Err(ToolError::Failed {
            tool,
            status: status.to_string(),
            stderr: truncate(&stderr, 512),
        })
    }
}

static MEDIA_ACTIVE: OnceLock<(Mutex<usize>, Condvar)> = OnceLock::new();

fn media_slots() -> &'static (Mutex<usize>, Condvar) {
    MEDIA_ACTIVE.get_or_init(|| (Mutex::new(0), Condvar::new()))
}

struct MediaPermit;

impl MediaPermit {
    fn acquire() -> Self {
        let (active, available) = media_slots();
        let mut count = active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        while *count >= MEDIA_PROCESS_LIMIT {
            count = available
                .wait(count)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        *count += 1;
        Self
    }
}

impl Drop for MediaPermit {
    fn drop(&mut self) {
        let (active, available) = media_slots();
        let mut count = active
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *count = count.saturating_sub(1);
        available.notify_one();
    }
}

pub fn application_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

fn unavailable() -> MediaTool {
    MediaTool {
        available: false,
        path: None,
        source: None,
    }
}

fn tool_info(path: PathBuf, source: &'static str) -> Option<MediaTool> {
    let metadata = std::fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    Some(MediaTool {
        available: true,
        path: Some(path.to_string_lossy().into_owned()),
        source: Some(source.into()),
    })
}

fn executable_name(stem: &str) -> OsString {
    if cfg!(windows) {
        OsString::from(format!("{stem}.exe"))
    } else {
        OsString::from(stem)
    }
}

fn truncate(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.into();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("openscp-media-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn bundled_tools_take_precedence_over_path() {
        let root = temp_dir("precedence");
        let bundled = root.join(FFMPEG_RELATIVE_PATH);
        fs::create_dir_all(bundled.parent().unwrap()).unwrap();
        fs::write(&bundled, b"bundled").unwrap();
        let path_dir = root.join("path");
        fs::create_dir_all(&path_dir).unwrap();
        fs::write(path_dir.join("ffmpeg.exe"), b"path").unwrap();
        let tools = resolve_tools(&root, Some(path_dir.as_os_str()));
        assert_eq!(tools.ffmpeg.source.as_deref(), Some("bundled"));
        assert_eq!(
            tools.ffmpeg.path.as_deref(),
            Some(bundled.to_string_lossy().as_ref())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn path_entries_are_checked_in_order_and_empty_entries_are_skipped() {
        let root = temp_dir("path");
        let first = root.join("first");
        let second = root.join("second");
        fs::create_dir_all(&second).unwrap();
        fs::write(second.join("ffmpeg.exe"), b"ffmpeg").unwrap();
        let path = std::env::join_paths([
            OsString::new(),
            first.as_os_str().to_owned(),
            second.as_os_str().to_owned(),
        ])
        .unwrap();
        let tools = resolve_tools(&root, Some(&path));
        assert_eq!(tools.ffmpeg.source.as_deref(), Some("path"));
        assert_eq!(
            tools.ffmpeg.path.as_deref(),
            Some(second.join("ffmpeg.exe").to_string_lossy().as_ref())
        );
        assert!(!tools.vgmstream.available);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_process_is_reported_without_panicking() {
        #[cfg(windows)]
        let (executable, args) = (
            PathBuf::from("cmd.exe"),
            vec![OsString::from("/C"), OsString::from("exit 7")],
        );
        #[cfg(not(windows))]
        let (executable, args) = (
            PathBuf::from("sh"),
            vec![OsString::from("-c"), OsString::from("exit 7")],
        );
        assert!(matches!(
            run_with_timeout("test-tool", &executable, &args, Duration::from_secs(1)),
            Err(ToolError::Failed { .. })
        ));
    }

    #[test]
    fn timed_out_process_is_terminated_and_reported() {
        #[cfg(windows)]
        let (executable, args) = (
            PathBuf::from("cmd.exe"),
            vec![
                OsString::from("/C"),
                OsString::from("ping 127.0.0.1 -n 6 > nul"),
            ],
        );
        #[cfg(not(windows))]
        let (executable, args) = (
            PathBuf::from("sh"),
            vec![OsString::from("-c"), OsString::from("sleep 2")],
        );
        assert!(matches!(
            run_with_timeout("test-tool", &executable, &args, Duration::from_millis(10)),
            Err(ToolError::Timeout { .. })
        ));
    }
    #[test]
    fn missing_tools_are_reported_without_error() {
        let root = temp_dir("missing");
        let tools = resolve_tools(&root, Some(OsStr::new("")));
        assert!(!tools.ffmpeg.available);
        assert!(!tools.vgmstream.available);
        let _ = fs::remove_dir_all(root);
    }
}
