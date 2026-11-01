//! Blender as an external asset-preparation service.
//!
//! OpenSCP never reimplements model or texture math here.  It only locates a
//! user-provided Blender, runs an internal Python script in background mode,
//! and returns the generated artifacts.  Project files, caches and manifests
//! remain owned by the caller.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

const NATIVE_EXPORT_SCRIPT: &str = include_str!("blender_scripts/native_export.py");

#[derive(Debug)]
pub(crate) enum NativeExportError {
    /// No bundled/configured plug-in, or Blender itself is unavailable.
    Unavailable(String),
    /// The plug-in explicitly rejects original B4 meshes.
    UnsupportedB4(String),
    Failed(String),
}

#[derive(Debug)]
pub(crate) struct NativeExport {
    pub glb: PathBuf,
    pub lotm: PathBuf,
}

/// Locate Blender without requiring a global PATH entry.
pub(crate) fn executable() -> PathBuf {
    if let Ok(value) = std::env::var("BLENDER_PATH") {
        let path = PathBuf::from(value.trim_matches('"'));
        if path.is_file() {
            return path;
        }
    }
    [
        PathBuf::from(r"D:\blender\blender.exe"),
        PathBuf::from(r"C:\Program Files\Blender Foundation\Blender\blender.exe"),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .unwrap_or_else(|| PathBuf::from("blender"))
}

/// Blender's Windows path handling rejects Rust's extended `\\?\` prefix.
pub(crate) fn path_arg(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(rest) = value.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_owned()
}

/// Run `script` in Blender background mode.  `script_args` follow the `--`
/// separator and are read by the script from `sys.argv[-N:]`.
pub(crate) fn run_script(script: &Path, script_args: &[String]) -> Result<Output, String> {
    let blender = executable();
    let mut command = Command::new(&blender);
    command
        .arg("--background")
        .arg("--python")
        .arg(path_arg(script))
        .arg("--")
        .args(script_args);
    command.output().map_err(|error| {
        format!(
            "Cannot start Blender at {}: {error}. Set BLENDER_PATH or install Blender.",
            blender.display()
        )
    })
}

/// Locate the bundled plug-in package.  `OPENSCP_BLENDER_ADDON` may point at
/// either the `ny_facade` directory or its parent.  A shipped build looks in
/// `<exe dir>/resources/blender/ny_facade`; no user add-on installation is
/// required.
pub(crate) fn addon_parent() -> Option<PathBuf> {
    let candidates = std::env::var("OPENSCP_BLENDER_ADDON")
        .ok()
        .map(PathBuf::from)
        .into_iter()
        .chain(
            std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf))
                .map(|dir| dir.join("resources").join("blender")),
        );
    for candidate in candidates {
        if candidate.join("ny_facade").is_dir() {
            return Some(candidate);
        }
        if candidate
            .file_name()
            .is_some_and(|name| name == "ny_facade")
        {
            return candidate.parent().map(Path::to_path_buf);
        }
    }
    None
}

/// Run the bundled plug-in's native exporter.  This adapter is deliberately
/// separate from mesh normalisation: it either succeeds with a native
/// GLB+LOTM pair, or reports an explicit unsupported/failure result.
pub(crate) fn export_native(
    addon_parent: &Path,
    source: &Path,
    work_dir: &Path,
) -> Result<NativeExport, NativeExportError> {
    let script = work_dir.join("native_export.py");
    std::fs::write(&script, NATIVE_EXPORT_SCRIPT)
        .map_err(|error| NativeExportError::Unavailable(error.to_string()))?;
    let out_glb = work_dir.join("asset.glb");
    let output = run_script(
        &script,
        &[path_arg(addon_parent), path_arg(source), path_arg(&out_glb)],
    )
    .map_err(NativeExportError::Unavailable)?;
    let lotm = out_glb.with_extension("lotm");
    if output.status.success() && out_glb.is_file() && lotm.is_file() {
        return Ok(NativeExport { glb: out_glb, lotm });
    }
    let tail = stderr_tail(&output, 20);
    if tail.contains("原工程 B4") || tail.contains("请选择插件制作的楼体") {
        return Err(NativeExportError::UnsupportedB4(tail));
    }
    Err(NativeExportError::Failed(tail))
}

/// Keep a bounded diagnostic tail instead of returning an unbounded log.
pub(crate) fn stderr_tail(output: &Output, limit: usize) -> String {
    let text = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<_> = text.lines().rev().take(limit).collect();
    lines.into_iter().rev().collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_windows_prefix_is_stripped_for_blender() {
        assert_eq!(
            path_arg(Path::new(r"\\?\D:\project\asset.blend")),
            r"D:\project\asset.blend"
        );
        assert_eq!(
            path_arg(Path::new(r"\\?\UNC\server\share\asset.blend")),
            r"\\server\share\asset.blend"
        );
    }
}
