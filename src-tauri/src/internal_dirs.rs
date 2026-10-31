//! 内部工作目录（2026-10-29 目录治理）：文档工作区与模组工作台的
//! `docs/`、`mods/` 不再由用户手动指定——启动时自动落在可执行文件
//! 目录下（绿色免安装版的"安装目录"），不可写（如 Program Files）
//! 时退回应用数据目录。两处消费方：lib.rs 启动初始化、历史数据同步命令。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sc_store::Store;
use tauri::{AppHandle, Manager, Runtime};

pub const DOCS_DIR_NAME: &str = "docs";
pub const MODS_DIR_NAME: &str = "mods";
const WRITE_PROBE_NAME: &str = ".openscp-write-probe";

fn dir_is_writable(dir: &Path) -> bool {
    let probe = dir.join(WRITE_PROBE_NAME);
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// 内部根目录：exe 目录优先（需可写），否则退回应用数据目录。目录会被创建。
fn internal_base<R: Runtime>(app: &AppHandle<R>) -> io::Result<PathBuf> {
    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
    {
        if exe_dir.is_dir() && dir_is_writable(&exe_dir) {
            return Ok(exe_dir);
        }
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| io::Error::other(error.to_string()))?;
    fs::create_dir_all(&data_dir)?;
    Ok(data_dir)
}

/// Persistent application root. Prefer the directory beside the executable so
/// portable/installed copies keep their database and workspaces together.
/// Fall back to Tauri's app-data directory only when the executable directory
/// is not writable (for example, an installation under Program Files).
pub fn persistent_root<R: Runtime>(app: &AppHandle<R>) -> io::Result<PathBuf> {
    internal_base(app)
}

fn ensure_subdir<R: Runtime>(
    app: &AppHandle<R>,
    name: &str,
) -> io::Result<PathBuf> {
    let dir = internal_base(app)?.join(name);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn copy_directory_contents(source: &Path, destination: &Path) -> io::Result<()> {
    if !source.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if destination_path.exists() {
            continue;
        }
        if source_path.is_dir() {
            copy_directory_contents(&source_path, &destination_path)?;
        } else {
            // Copy instead of moving.  The legacy app-data directory may be
            // protected by Windows ACLs that allow reads but deny deletion.
            // Keeping the source also makes migration recoverable if startup
            // is interrupted halfway through.
            fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

/// 内部文档工作区目录（不存在则创建）。
pub fn docs_dir(app: &AppHandle) -> io::Result<PathBuf> {
    ensure_subdir(app, DOCS_DIR_NAME)
}

/// 内部模组项目目录（不存在则创建）。
pub fn mods_dir(app: &AppHandle) -> io::Result<PathBuf> {
    ensure_subdir(app, MODS_DIR_NAME)
}

/// 启动初始化：确保两个内部目录存在；尚未配置的工作区/模组根自动落到
/// 内部目录。历史用户已配置的路径保持不动（由页面上的同步按钮迁移），
/// 这样迁移前数据不会"看起来消失"。
pub fn ensure_internal_roots(app: &AppHandle, store: &Store) -> Result<(), sc_store::StoreError> {
    let docs = docs_dir(app)?;
    let mods = mods_dir(app)?;
    let legacy_root = app
        .path()
        .app_data_dir()
        .map_err(|error| io::Error::other(error.to_string()))?;
    let legacy_docs = legacy_root.join(DOCS_DIR_NAME);
    let legacy_mods = legacy_root.join(MODS_DIR_NAME);
    if store.workspace_config()?.is_none() {
        store.set_workspace_config(&docs.to_string_lossy())?;
    } else if store
        .workspace_config()?
        .is_some_and(|config| config.root_path == legacy_docs.to_string_lossy())
    {
        copy_directory_contents(&legacy_docs, &docs)?;
        store.set_workspace_config(&docs.to_string_lossy())?;
    }
    let studio = store.studio_config()?;
    if studio.mod_root.is_none() {
        store.set_studio_config(Some(&mods.to_string_lossy()), studio.onboarding_completed)?;
    } else if studio.mod_root.as_deref() == Some(legacy_mods.to_string_lossy().as_ref()) {
        copy_directory_contents(&legacy_mods, &mods)?;
        store.set_studio_config(Some(&mods.to_string_lossy()), studio.onboarding_completed)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_probe_detects_writable_dir() {
        let dir = std::env::temp_dir().join(format!("openscp-probe-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(dir_is_writable(&dir));
        assert!(!dir.join(WRITE_PROBE_NAME).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
