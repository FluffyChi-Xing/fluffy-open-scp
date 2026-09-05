use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::activity::CommandError;

const MAX_DEPTH: usize = 4;
const MAX_DIRECT_ENTRIES: usize = 4096;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryRequest {
    pub root: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameFolder {
    pub path: String,
    pub name: String,
    pub children: Vec<GameFolder>,
    pub files: Vec<PackageFile>,
    pub package_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFile {
    pub path: String,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, thiserror::Error)]
enum BrowserError {
    #[error("directory path is empty")]
    EmptyPath,
    #[error("path is not a directory")]
    NotDirectory,
    #[error("directory is a symbolic link or reparse point")]
    Symlink,
    #[error("directory scan exceeded the entry limit")]
    EntryLimit,
    #[error("directory scan failed: {0}")]
    Io(#[from] io::Error),
}

impl BrowserError {
    fn code(&self) -> &'static str {
        match self {
            Self::EmptyPath => "invalid_path",
            Self::NotDirectory => "invalid_type",
            Self::Symlink => "path_forbidden",
            Self::EntryLimit => "limit_exceeded",
            Self::Io(error) if error.kind() == io::ErrorKind::PermissionDenied => "access_denied",
            Self::Io(error) if error.kind() == io::ErrorKind::NotFound => "not_found",
            Self::Io(_) => "io",
        }
    }
}

impl From<BrowserError> for CommandError {
    fn from(error: BrowserError) -> Self {
        CommandError::new(error.code(), error.to_string())
    }
}

fn canonical_directory(value: &str) -> Result<PathBuf, BrowserError> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(BrowserError::EmptyPath);
    }
    let path = PathBuf::from(value);
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.file_type().is_symlink() {
        return Err(BrowserError::Symlink);
    }
    if !metadata.is_dir() {
        return Err(BrowserError::NotDirectory);
    }
    Ok(fs::canonicalize(path)?)
}

fn is_package(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("package"))
}

fn directory_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn scan_directory(path: &Path, depth: usize) -> Result<GameFolder, BrowserError> {
    let mut children = Vec::new();
    let mut files = Vec::new();
    let mut package_count = 0;
    let mut entries = 0;
    for entry in fs::read_dir(path)? {
        entries += 1;
        if entries > MAX_DIRECT_ENTRIES {
            return Err(BrowserError::EntryLimit);
        }
        let entry = entry?;
        let entry_path = entry.path();
        let metadata = fs::symlink_metadata(&entry_path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_file() {
            let is_package = is_package(&entry_path);
            if is_package {
                package_count += 1;
            }
            files.push(PackageFile {
                path: entry_path.to_string_lossy().into_owned(),
                name: entry.file_name().to_string_lossy().into_owned(),
                size: metadata.len(),
            });
        } else if metadata.is_dir() && depth < MAX_DEPTH {
            children.push(scan_directory(&entry_path, depth + 1)?);
        }
    }
    files.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.path.cmp(&right.path))
    });
    children.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(GameFolder {
        path: path.to_string_lossy().into_owned(),
        name: directory_name(path),
        children,
        files,
        package_count,
    })
}

fn list_packages(path: &Path) -> Result<Vec<PackageFile>, BrowserError> {
    let mut files = Vec::new();
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(BrowserError::Symlink);
    }
    if !metadata.is_dir() {
        return Err(BrowserError::NotDirectory);
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        let metadata = fs::symlink_metadata(&entry_path)?;
        if metadata.file_type().is_symlink() || !metadata.is_file() || !is_package(&entry_path) {
            continue;
        }
        files.push(PackageFile {
            path: fs::canonicalize(&entry_path)?
                .to_string_lossy()
                .into_owned(),
            name: entry.file_name().to_string_lossy().into_owned(),
            size: metadata.len(),
        });
        if files.len() > MAX_DIRECT_ENTRIES {
            return Err(BrowserError::EntryLimit);
        }
    }
    files.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(files)
}

#[tauri::command]
pub async fn list_game_tree(request: DirectoryRequest) -> Result<Vec<GameFolder>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = canonical_directory(&request.root).map_err(CommandError::from)?;
        let tree = scan_directory(&root, 0).map_err(CommandError::from)?;
        Ok(vec![tree])
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn list_package_files(
    request: DirectoryRequest,
) -> Result<Vec<PackageFile>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let folder = canonical_directory(&request.root).map_err(CommandError::from)?;
        list_packages(&folder).map_err(CommandError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn temp_dir(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("openscp-browser-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn lists_only_direct_package_files_in_stable_order() {
        let root = temp_dir("files");
        File::create(root.join("z.PACKAGE")).unwrap();
        File::create(root.join("a.package")).unwrap();
        File::create(root.join("ignore.txt")).unwrap();
        let files = list_packages(&root).unwrap();
        assert_eq!(
            files
                .iter()
                .map(|file| file.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a.package", "z.PACKAGE"]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scans_nested_directories() {
        let root = temp_dir("tree");
        let nested = root.join("SimCityData");
        fs::create_dir_all(&nested).unwrap();
        File::create(nested.join("DLC0.package")).unwrap();
        let tree = scan_directory(&root, 0).unwrap();
        assert_eq!(tree.children[0].package_count, 1);
        let _ = fs::remove_dir_all(root);
    }
}
