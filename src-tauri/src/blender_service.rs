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
const EXTRACT_SCRIPT: &str = include_str!("blender_scripts/extract_blend.py");

/// The `.blend` import pipeline is native Rust: Blender only runs our reader
/// script, and `blend_native::export` produces the LOTM.  The atlas catalog
/// and pack images are data resources; no plug-in code runs at import time.
pub(crate) fn atlas_dir() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(value) = std::env::var("OPENSCP_BLENDER_ADDON") {
        let value = value.trim_matches('"').to_string();
        candidates.push(PathBuf::from(&value).join("assets").join("packs"));
        candidates.push(PathBuf::from(&value));
    }
    candidates.extend(
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .map(|dir| dir.join("resources").join("blender").join("ny_facade").join("assets").join("packs")),
    );
    if let Ok(appdata) = std::env::var("APPDATA") {
        let blender = PathBuf::from(appdata)
            .join("Blender Foundation")
            .join("Blender");
        if let Ok(versions) = std::fs::read_dir(&blender) {
            for version in versions.flatten() {
                candidates.push(
                    version
                        .path()
                        .join("scripts")
                        .join("addons")
                        .join("ny_facade")
                        .join("assets")
                        .join("packs"),
                );
            }
        }
    }
    candidates
        .into_iter()
        .find(|candidate| candidate.join("catalog.json").is_file())
}

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
    /// Preview GLB written by the exporter.  Kept for diagnostics only: the
    /// project persists the LOTM and never imports this GLB.
    #[allow(dead_code)]
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

/// User-facing hint when the native exporter is not available.
pub(crate) const MISSING_ADDON_HINT: &str =
    "未找到 SC5 Asset Editor 导出器 ny_facade：请运行 scripts/stage-blender-addon.ps1，或设置 OPENSCP_BLENDER_ADDON 指向插件目录";

/// Locate the plug-in package.  `OPENSCP_BLENDER_ADDON` may point at either
/// the `ny_facade` directory or its parent.  A shipped build looks in
/// `<exe dir>/resources/blender/ny_facade`; Blender's user add-on directory is
/// also scanned because the SC5 Asset Editor is commonly installed there.
pub(crate) fn addon_parent() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(value) = std::env::var("OPENSCP_BLENDER_ADDON") {
        candidates.push(PathBuf::from(value.trim_matches('"')));
    }
    candidates.extend(
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .map(|dir| dir.join("resources").join("blender")),
    );
    // Blender installs user add-ons under
    // `%APPDATA%/Blender Foundation/Blender/<version>/scripts/addons`.
    if let Ok(appdata) = std::env::var("APPDATA") {
        let blender = PathBuf::from(appdata)
            .join("Blender Foundation")
            .join("Blender");
        if let Ok(versions) = std::fs::read_dir(&blender) {
            for version in versions.flatten() {
                candidates.push(version.path().join("scripts").join("addons"));
            }
        }
    }
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

/// Native Rust export: read the .blend into a neutral bundle, then run the
/// Rust exporter.  Returns the LOTM bytes.
pub(crate) fn export_native_rust(
    atlas: &Path,
    source: &Path,
    work_dir: &Path,
) -> Result<Vec<u8>, String> {
    let script = work_dir.join("extract_blend.py");
    std::fs::write(&script, EXTRACT_SCRIPT).map_err(|error| error.to_string())?;
    let bundle_path = work_dir.join("bundle.json");
    let started = std::time::Instant::now();
    crate::blend_native::trace::emit(
        "blender_start",
        serde_json::json!({
            "addon": path_arg(atlas),
            "source": path_arg(source),
            "output": path_arg(&bundle_path),
        }),
    );
    let output = run_script(
        &script,
        &[path_arg(source), path_arg(&bundle_path)],
    )?;
    crate::blend_native::trace::emit(
        "blender_done",
        serde_json::json!({
            "status": output.status.code(),
            "elapsed_ms": started.elapsed().as_millis(),
            "stderr_tail": stderr_tail(&output, 8),
        }),
    );
    let bundle_bytes = std::fs::read(&bundle_path).map_err(|error| error.to_string())?;
    let bundle = crate::blend_native::dump::Bundle::parse(&bundle_bytes)
        .map_err(|error| format!("bundle parse failed: {error}"))?;
    let store = crate::blend_native::resources::PackStore::new(atlas);
    let catalog = crate::blend_native::catalog::Catalog::parse(
        &std::fs::read(atlas.join("catalog.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("catalog parse failed: {error}"))?;
    let index = catalog.index();
    let report = crate::blend_native::export::export(&crate::blend_native::export::ExportInput {
        bundle: &bundle,
        catalog: &catalog,
        index: &index,
        store: &store,
    })?;
    Ok(report.lotm)
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
    let started = std::time::Instant::now();
    crate::blend_native::trace::emit(
        "blender_start",
        serde_json::json!({
            "addon": path_arg(addon_parent),
            "source": path_arg(source),
            "output": path_arg(&out_glb),
        }),
    );
    let output = match run_script(
        &script,
        &[path_arg(addon_parent), path_arg(source), path_arg(&out_glb)],
    ) {
        Ok(output) => output,
        Err(error) => {
            crate::blend_native::trace::emit(
                "blender_unavailable",
                serde_json::json!({ "detail": error }),
            );
            return Err(NativeExportError::Unavailable(error));
        }
    };
    crate::blend_native::trace::emit(
        "blender_done",
        serde_json::json!({
            "status": output.status.code(),
            "elapsed_ms": started.elapsed().as_millis(),
            "stderr_tail": stderr_tail(&output, 8),
        }),
    );
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
