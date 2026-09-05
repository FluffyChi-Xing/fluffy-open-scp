use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

pub const VGMSTREAM_RELATIVE_PATH: &str = "Tools/vgmstream/vgmstream-cli.exe";
pub const FFMPEG_RELATIVE_PATH: &str = "Tools/ffmpeg/ffmpeg.exe";

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
}

pub fn resolve_tools(application_dir: &Path, path: Option<&OsStr>) -> MediaTools {
    let vgmstream_path = application_dir.join(VGMSTREAM_RELATIVE_PATH);
    let vgmstream = tool_info(vgmstream_path, "bundled").unwrap_or_else(unavailable);
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
    let output = Command::new(executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|source| ToolError::Spawn { tool, source })?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.lines().take(4).collect::<Vec<_>>().join(" ");
        Err(ToolError::Failed {
            tool,
            status: output.status.to_string(),
            stderr: truncate(&stderr, 512),
        })
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
    fn missing_tools_are_reported_without_error() {
        let root = temp_dir("missing");
        let tools = resolve_tools(&root, Some(OsStr::new("")));
        assert!(!tools.ffmpeg.available);
        assert!(!tools.vgmstream.available);
        let _ = fs::remove_dir_all(root);
    }
}
