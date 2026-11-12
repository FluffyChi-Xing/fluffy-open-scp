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

const EXTRACT_SCRIPT: &str = include_str!("blender_scripts/extract_blend.py");

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
    run_script_with_startup(script, script_args, &[])
}

fn run_script_with_startup(
    script: &Path,
    script_args: &[String],
    startup_args: &[&str],
) -> Result<Output, String> {
    let blender = executable();
    let mut command = Command::new(&blender);
    command
        .arg("--background")
        .args(startup_args)
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
            "atlas": path_arg(atlas),
            "source": path_arg(source),
            "output": path_arg(&bundle_path),
        }),
    );
    let output = run_script_with_startup(
        &script,
        &[path_arg(source), path_arg(&bundle_path)],
        &["--factory-startup", "--disable-autoexec", "--python-exit-code", "1"],
    )?;
    crate::blend_native::trace::emit(
        "blender_done",
        serde_json::json!({
            "status": output.status.code(),
            "elapsed_ms": started.elapsed().as_millis(),
            "stderr_tail": stderr_tail(&output, 8),
        }),
    );
    if !output.status.success() {
        return Err(format!(
            "Blender reader failed: {}",
            stderr_tail(&output, 16)
        ));
    }
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
