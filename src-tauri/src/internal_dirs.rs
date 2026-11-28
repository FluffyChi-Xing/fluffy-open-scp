//! 内部工作目录（2026-10-29 目录治理）：文档工作区与模组工作台的
//! 文档目录沿用应用管理逻辑；mod 项目唯一权威为 D:/openscp/mods，
//! 数据库唯一权威为 D:/openscp/openscp.db。开发构建不得另建项目数据源。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use sc_store::Store;
use tauri::{AppHandle, Manager, Runtime};

pub const DOCS_DIR_NAME: &str = "docs";
pub const MODS_DIR_NAME: &str = "mods";
/// User-mandated authority shared by development and installed builds.
pub const MANAGED_APP_ROOT: &str = r"D:\openscp";

fn managed_mods_path() -> PathBuf {
    PathBuf::from(MANAGED_APP_ROOT).join(MODS_DIR_NAME)
}

fn normalize_mod_root(store: &Store) -> Result<(), sc_store::StoreError> {
    let studio = store.studio_config()?;
    let root = managed_mods_path();
    if studio.mod_root.as_deref() != Some(root.to_string_lossy().as_ref()) {
        store.set_studio_config(Some(&root.to_string_lossy()), studio.onboarding_completed)?;
    }
    Ok(())
}
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

/// All builds use the installed database; executable-relative debug databases
/// are not authoritative and must never provide the project list.
pub fn persistent_root<R: Runtime>(_app: &AppHandle<R>) -> io::Result<PathBuf> {
    let root = PathBuf::from(MANAGED_APP_ROOT);
    fs::create_dir_all(&root)?;
    Ok(root)
}

fn ensure_subdir<R: Runtime>(app: &AppHandle<R>, name: &str) -> io::Result<PathBuf> {
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

/// 唯一模组项目目录（不随可执行文件位置或历史配置改变）。
pub fn mods_dir(app: &AppHandle) -> io::Result<PathBuf> {
    let _ = app;
    let root = managed_mods_path();
    fs::create_dir_all(&root)?;
    Ok(root)
}

/// 初始化文档目录，并强制纠正历史 mod 配置到唯一受管目录。
pub fn ensure_internal_roots(app: &AppHandle, store: &Store) -> Result<(), sc_store::StoreError> {
    let docs = docs_dir(app)?;
    mods_dir(app)?;
    let legacy_root = app
        .path()
        .app_data_dir()
        .map_err(|error| io::Error::other(error.to_string()))?;
    let legacy_docs = legacy_root.join(DOCS_DIR_NAME);
    if store.workspace_config()?.is_none() {
        store.set_workspace_config(&docs.to_string_lossy())?;
    } else if store
        .workspace_config()?
        .is_some_and(|config| config.root_path == legacy_docs.to_string_lossy())
    {
        copy_directory_contents(&legacy_docs, &docs)?;
        store.set_workspace_config(&docs.to_string_lossy())?;
    }
    // Repair stale roots without merging non-authoritative project folders.
    normalize_mod_root(store)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_mod_root_repairs_debug_and_legacy_configuration_without_resetting_onboarding() {
        for wrong in [
            r"D:\rust\packages\fluffy-open-scp\target\debug\mods",
            r"C:\legacy\mods",
        ] {
            let store = Store::open(":memory:").unwrap();
            store.set_studio_config(Some(wrong), true).unwrap();
            normalize_mod_root(&store).unwrap();
            let config = store.studio_config().unwrap();
            assert_eq!(config.mod_root.as_deref(), Some(r"D:\openscp\mods"));
            assert!(config.onboarding_completed);
            normalize_mod_root(&store).unwrap();
            assert_eq!(store.studio_config().unwrap().mod_root, config.mod_root);
        }
    }

    #[test]
    fn write_probe_detects_writable_dir() {
        let dir = std::env::temp_dir().join(format!("openscp-probe-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(dir_is_writable(&dir));
        assert!(!dir.join(WRITE_PROBE_NAME).exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
