use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use sc_store::{FolderCacheEntry, Store, StoreError, WorkspaceConfig};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{State, command};

use crate::activity::{AppState, CommandError};

pub const MAX_MARKDOWN_BYTES: usize = 4 * 1024 * 1024;
/// Code 工作台文本读取上限（与 Markdown 同数量级的防失控闸）。
pub const MAX_CODE_TEXT_BYTES: usize = 4 * 1024 * 1024;
/// Code 树条目预算：模组目录可能数千文件，超出即截断并回报 truncated。
const MAX_CODE_TREE_ENTRIES: usize = 5000;
const MAX_CODE_TREE_DEPTH: usize = 16;
const MAX_PATH_COMPONENT_BYTES: usize = 128;
const MAX_RELATIVE_PATH_BYTES: usize = 512;

#[derive(Debug)]
pub struct WorkspaceManager {
    lock: Mutex<()>,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        Self {
            lock: Mutex::new(()),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, ()>, WorkspaceError> {
        self.lock.lock().map_err(|_| WorkspaceError::StatePoisoned)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    pub configured: bool,
    pub root_path: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEntry {
    pub relative_path: String,
    pub kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownDocument {
    pub relative_path: String,
    pub content: String,
    pub size: usize,
    pub revision: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRootRequest {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelativePathRequest {
    pub relative_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteMarkdownRequest {
    pub relative_path: String,
    pub content: String,
    pub expected_revision: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMarkdownRequest {
    pub relative_path: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameRequest {
    pub relative_path: String,
    pub new_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveRequest {
    pub relative_path: String,
    pub target_directory: String,
}

#[derive(Debug, thiserror::Error)]
enum WorkspaceError {
    #[error("workspace has not been configured")]
    NotConfigured,
    #[error("studio mod root has not been configured")]
    ModRootNotConfigured,
    #[error("workspace path is invalid: {0}")]
    InvalidPath(String),
    #[error("workspace root is unavailable")]
    RootUnavailable,
    #[error("workspace path escapes the configured root")]
    OutsideRoot,
    #[error("symbolic links and reparse points are not allowed in the workspace path")]
    Symlink,
    #[error("workspace path was not found")]
    NotFound,
    #[error("workspace path is not a directory")]
    NotDirectory,
    #[error("workspace path is not a regular file")]
    NotFile,
    #[error("only Markdown files are supported")]
    NotMarkdown,
    #[error("Markdown content exceeds the {0} byte limit")]
    TooLarge(usize),
    #[error("Markdown revision conflict")]
    RevisionConflict,
    #[error("Markdown file already exists")]
    AlreadyExists,
    #[error("workspace state is unavailable")]
    StatePoisoned,
    #[error("workspace io error: {0}")]
    Io(#[from] io::Error),
    #[error("workspace store error: {0}")]
    Store(#[from] StoreError),
    #[error("Markdown is not valid UTF-8")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}

impl WorkspaceError {
    fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured => "workspace_not_configured",
            Self::ModRootNotConfigured => "mod_root_not_configured",
            Self::RootUnavailable => "mod_root_unavailable",
            Self::InvalidPath(_) | Self::NotMarkdown => "invalid_path",
            Self::OutsideRoot | Self::Symlink => "path_forbidden",
            Self::NotFound => "not_found",
            Self::NotDirectory | Self::NotFile => "invalid_type",
            Self::TooLarge(_) => "limit_exceeded",
            Self::RevisionConflict => "revision_conflict",
            Self::AlreadyExists => "already_exists",
            Self::StatePoisoned => "internal_error",
            Self::Io(_) | Self::Store(_) | Self::InvalidUtf8(_) => "workspace_error",
        }
    }
}

impl From<WorkspaceError> for CommandError {
    fn from(error: WorkspaceError) -> Self {
        CommandError::new(error.code(), error.to_string())
    }
}

fn configured_root(store: &Store) -> Result<(PathBuf, WorkspaceConfig), WorkspaceError> {
    let config = store
        .workspace_config()?
        .ok_or(WorkspaceError::NotConfigured)?;
    let root = PathBuf::from(&config.root_path);
    let metadata = fs::symlink_metadata(&root).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            WorkspaceError::RootUnavailable
        } else {
            WorkspaceError::Io(error)
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(WorkspaceError::Symlink);
    }
    if !metadata.is_dir() {
        return Err(WorkspaceError::NotDirectory);
    }
    let canonical = fs::canonicalize(root)?;
    Ok((canonical, config))
}

fn validate_relative_path(value: &str, allow_empty: bool) -> Result<PathBuf, WorkspaceError> {
    if value.is_empty() {
        if allow_empty {
            return Ok(PathBuf::new());
        }
        return Err(WorkspaceError::InvalidPath("path cannot be empty".into()));
    }
    if value.len() > MAX_RELATIVE_PATH_BYTES
        || value.contains('\0')
        || value.contains('\\')
        || value.contains(':')
    {
        return Err(WorkspaceError::InvalidPath(
            "path must be a short slash-separated relative path".into(),
        ));
    }
    let mut path = PathBuf::new();
    for component in value.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(WorkspaceError::InvalidPath(
                "path contains an empty or traversal component".into(),
            ));
        }
        if component.len() > MAX_PATH_COMPONENT_BYTES {
            return Err(WorkspaceError::InvalidPath(
                "path component is too long".into(),
            ));
        }
        path.push(component);
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(WorkspaceError::InvalidPath(
            "path contains a non-normal component".into(),
        ));
    }
    Ok(path)
}

fn markdown_path(value: &str) -> Result<PathBuf, WorkspaceError> {
    let path = validate_relative_path(value, false)?;
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().ends_with(".md"))
    {
        return Err(WorkspaceError::NotMarkdown);
    }
    Ok(path)
}

fn existing_path(root: &Path, relative: &Path) -> Result<PathBuf, WorkspaceError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(WorkspaceError::InvalidPath(
                "path contains a non-normal component".into(),
            ));
        };
        current.push(name);
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                WorkspaceError::NotFound
            } else {
                WorkspaceError::Io(error)
            }
        })?;
        if metadata.file_type().is_symlink() {
            return Err(WorkspaceError::Symlink);
        }
        let canonical = fs::canonicalize(&current)?;
        if !canonical.starts_with(root) {
            return Err(WorkspaceError::OutsideRoot);
        }
        current = canonical;
    }
    Ok(current)
}

fn existing_parent(root: &Path, relative: &Path) -> Result<PathBuf, WorkspaceError> {
    let Some(parent) = relative
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(root.to_path_buf());
    };
    let path = existing_path(root, parent)?;
    if !path.is_dir() {
        return Err(WorkspaceError::NotDirectory);
    }
    Ok(path)
}

fn revision(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn read_markdown_file(path: &Path) -> Result<(String, usize, String), WorkspaceError> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(WorkspaceError::NotFile);
    }
    if metadata.len() > MAX_MARKDOWN_BYTES as u64 {
        return Err(WorkspaceError::TooLarge(MAX_MARKDOWN_BYTES));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize + 1);
    File::open(path)?
        .take((MAX_MARKDOWN_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_MARKDOWN_BYTES {
        return Err(WorkspaceError::TooLarge(MAX_MARKDOWN_BYTES));
    }
    let size = bytes.len();
    let hash = revision(&bytes);
    let content = String::from_utf8(bytes)?;
    Ok((content, size, hash))
}

fn scan_entries(
    root: &Path,
    current: &Path,
    relative: &str,
    entries: &mut Vec<WorkspaceEntry>,
) -> Result<(), WorkspaceError> {
    let mut children = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let path = child.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let name = child.file_name().to_string_lossy().into_owned();
        let is_markdown = name.to_ascii_lowercase().ends_with(".md");
        let child_relative = if relative.is_empty() {
            name
        } else {
            format!("{relative}/{name}")
        };
        if metadata.is_dir() {
            let canonical = fs::canonicalize(&path)?;
            if !canonical.starts_with(root) {
                continue;
            }
            entries.push(WorkspaceEntry {
                relative_path: child_relative.clone(),
                kind: "folder",
            });
            scan_entries(root, &canonical, &child_relative, entries)?;
        } else if metadata.is_file() && is_markdown {
            entries.push(WorkspaceEntry {
                relative_path: child_relative,
                kind: "file",
            });
        }
    }
    Ok(())
}

fn current_time_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn refresh_cache(store: &Store, root: &Path) -> Result<Vec<WorkspaceEntry>, WorkspaceError> {
    let mut entries = Vec::new();
    scan_entries(root, root, "", &mut entries)?;
    let folders = entries
        .iter()
        .filter(|entry| entry.kind == "folder")
        .map(|entry| FolderCacheEntry {
            relative_path: entry.relative_path.clone(),
            readme_relative_path: None,
            last_seen_at: current_time_millis(),
        })
        .collect::<Vec<_>>();
    store.replace_folder_cache(&folders)?;
    Ok(entries)
}

fn write_atomic(target: &Path, bytes: &[u8], replace: bool) -> Result<(), WorkspaceError> {
    crate::atomic_fs::write_atomic(target, bytes, replace).map_err(WorkspaceError::from)
}

#[command]
pub async fn workspace_get(state: State<'_, AppState>) -> Result<WorkspaceStatus, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let config = store.workspace_config().map_err(CommandError::from)?;
        let Some(config) = config else {
            return Ok(WorkspaceStatus {
                configured: false,
                root_path: None,
                available: false,
            });
        };
        let root = PathBuf::from(&config.root_path);
        let available = fs::symlink_metadata(&root)
            .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink());
        Ok(WorkspaceStatus {
            configured: true,
            root_path: Some(config.root_path),
            available,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[command]
pub async fn workspace_set_root(
    state: State<'_, AppState>,
    request: WorkspaceRootRequest,
) -> Result<WorkspaceStatus, CommandError> {
    let manager = Arc::clone(&state.workspace);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = manager.lock().map_err(CommandError::from)?;
        let path = PathBuf::from(&request.path);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?;
        if metadata.file_type().is_symlink() {
            return Err(CommandError::from(WorkspaceError::Symlink));
        }
        if !metadata.is_dir() {
            return Err(CommandError::from(WorkspaceError::NotDirectory));
        }
        let canonical = fs::canonicalize(path)
            .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?;
        let config = store
            .set_workspace_config(&canonical.to_string_lossy())
            .map_err(CommandError::from)?;
        Ok(WorkspaceStatus {
            configured: true,
            root_path: Some(config.root_path),
            available: true,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[command]
pub async fn workspace_list(
    state: State<'_, AppState>,
) -> Result<Vec<WorkspaceEntry>, CommandError> {
    let manager = Arc::clone(&state.workspace);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = manager.lock().map_err(CommandError::from)?;
        let (root, _) = configured_root(&store).map_err(CommandError::from)?;
        refresh_cache(&store, &root).map_err(CommandError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[command]
pub async fn workspace_create_folder(
    state: State<'_, AppState>,
    request: RelativePathRequest,
) -> Result<Vec<WorkspaceEntry>, CommandError> {
    let manager = Arc::clone(&state.workspace);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = manager.lock().map_err(CommandError::from)?;
        let (root, _) = configured_root(&store).map_err(CommandError::from)?;
        let relative =
            validate_relative_path(&request.relative_path, false).map_err(CommandError::from)?;
        let mut current = root.clone();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(CommandError::from(WorkspaceError::InvalidPath(
                    "invalid folder path".into(),
                )));
            };
            let next = current.join(name);
            match fs::symlink_metadata(&next) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(CommandError::from(WorkspaceError::Symlink));
                }
                Ok(metadata) if !metadata.is_dir() => {
                    return Err(CommandError::from(WorkspaceError::NotDirectory));
                }
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(&next)
                    .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?,
                Err(error) => return Err(CommandError::from(WorkspaceError::Io(error))),
            }
            let canonical = fs::canonicalize(&next)
                .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?;
            if !canonical.starts_with(&root) {
                return Err(CommandError::from(WorkspaceError::OutsideRoot));
            }
            current = canonical;
        }
        refresh_cache(&store, &root).map_err(CommandError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[command]
pub async fn workspace_read_markdown(
    state: State<'_, AppState>,
    request: RelativePathRequest,
) -> Result<MarkdownDocument, CommandError> {
    let manager = Arc::clone(&state.workspace);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = manager.lock().map_err(CommandError::from)?;
        let (root, _) = configured_root(&store).map_err(CommandError::from)?;
        let relative = markdown_path(&request.relative_path).map_err(CommandError::from)?;
        let path = existing_path(&root, &relative).map_err(CommandError::from)?;
        let (content, size, revision) = read_markdown_file(&path).map_err(CommandError::from)?;
        Ok(MarkdownDocument {
            relative_path: request.relative_path,
            content,
            size,
            revision,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[command]
pub async fn workspace_write_markdown(
    state: State<'_, AppState>,
    request: WriteMarkdownRequest,
) -> Result<MarkdownDocument, CommandError> {
    let manager = Arc::clone(&state.workspace);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = manager.lock().map_err(CommandError::from)?;
        let (root, _) = configured_root(&store).map_err(CommandError::from)?;
        let relative = markdown_path(&request.relative_path).map_err(CommandError::from)?;
        let parent = existing_parent(&root, &relative).map_err(CommandError::from)?;
        let target = parent.join(
            relative
                .file_name()
                .ok_or_else(|| CommandError::from(WorkspaceError::NotMarkdown))?,
        );
        let existing = match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(CommandError::from(WorkspaceError::Symlink));
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(CommandError::from(WorkspaceError::NotFile));
            }
            Ok(_) => Some(read_markdown_file(&target).map_err(CommandError::from)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(CommandError::from(WorkspaceError::Io(error))),
        };
        if let Some((_, _, actual)) = &existing {
            if request.expected_revision.as_deref() != Some(actual.as_str()) {
                return Err(CommandError::from(WorkspaceError::RevisionConflict));
            }
        } else if request.expected_revision.is_some() {
            return Err(CommandError::from(WorkspaceError::RevisionConflict));
        }
        let bytes = request.content.as_bytes();
        if bytes.len() > MAX_MARKDOWN_BYTES {
            return Err(CommandError::from(WorkspaceError::TooLarge(
                MAX_MARKDOWN_BYTES,
            )));
        }
        write_atomic(&target, bytes, existing.is_some()).map_err(CommandError::from)?;
        Ok(MarkdownDocument {
            relative_path: request.relative_path,
            content: request.content.clone(),
            size: bytes.len(),
            revision: revision(bytes),
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[command]
pub async fn workspace_create_markdown(
    state: State<'_, AppState>,
    request: CreateMarkdownRequest,
) -> Result<MarkdownDocument, CommandError> {
    let manager = Arc::clone(&state.workspace);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = manager.lock().map_err(CommandError::from)?;
        let (root, _) = configured_root(&store).map_err(CommandError::from)?;
        let relative = markdown_path(&request.relative_path).map_err(CommandError::from)?;
        let parent = existing_parent(&root, &relative).map_err(CommandError::from)?;
        let target = parent.join(
            relative
                .file_name()
                .ok_or_else(|| CommandError::from(WorkspaceError::NotMarkdown))?,
        );
        if fs::symlink_metadata(&target).is_ok() {
            return Err(CommandError::from(WorkspaceError::AlreadyExists));
        }
        let bytes = request.content.as_bytes();
        if bytes.len() > MAX_MARKDOWN_BYTES {
            return Err(CommandError::from(WorkspaceError::TooLarge(
                MAX_MARKDOWN_BYTES,
            )));
        }
        write_atomic(&target, bytes, false).map_err(CommandError::from)?;
        Ok(MarkdownDocument {
            relative_path: request.relative_path,
            content: request.content.clone(),
            size: bytes.len(),
            revision: revision(bytes),
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

fn resolve_entry(root: &Path, relative: &str) -> Result<PathBuf, WorkspaceError> {
    let path = validate_relative_path(relative, false)?;
    let full = existing_path(root, &path)?;
    Ok(full)
}

#[command]
pub async fn workspace_rename(
    state: State<'_, AppState>,
    request: RenameRequest,
) -> Result<Vec<WorkspaceEntry>, CommandError> {
    let manager = Arc::clone(&state.workspace);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = manager.lock().map_err(CommandError::from)?;
        let (root, _) = configured_root(&store).map_err(CommandError::from)?;
        let source = resolve_entry(&root, &request.relative_path).map_err(CommandError::from)?;
        let name = validate_relative_path(&request.new_name, false).map_err(CommandError::from)?;
        if name.components().count() != 1 {
            return Err(CommandError::from(WorkspaceError::InvalidPath(
                "name must be a single path component".into(),
            )));
        }
        let parent = source.parent().ok_or_else(|| {
            CommandError::from(WorkspaceError::InvalidPath("entry has no parent".into()))
        })?;
        let target = parent.join(&name);
        if fs::symlink_metadata(&target).is_ok() {
            return Err(CommandError::from(WorkspaceError::AlreadyExists));
        }
        fs::rename(&source, &target)
            .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?;
        refresh_cache(&store, &root).map_err(CommandError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[command]
pub async fn workspace_move(
    state: State<'_, AppState>,
    request: MoveRequest,
) -> Result<Vec<WorkspaceEntry>, CommandError> {
    let manager = Arc::clone(&state.workspace);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = manager.lock().map_err(CommandError::from)?;
        let (root, _) = configured_root(&store).map_err(CommandError::from)?;
        let source = resolve_entry(&root, &request.relative_path).map_err(CommandError::from)?;
        let target_directory =
            validate_relative_path(&request.target_directory, true).map_err(CommandError::from)?;
        let target_directory = if target_directory.as_os_str().is_empty() {
            root.clone()
        } else {
            existing_path(&root, &target_directory).map_err(CommandError::from)?
        };
        if !target_directory.is_dir() {
            return Err(CommandError::from(WorkspaceError::NotDirectory));
        }
        let name = source.file_name().ok_or_else(|| {
            CommandError::from(WorkspaceError::InvalidPath("entry has no name".into()))
        })?;
        let target = target_directory.join(name);
        if source.starts_with(&target) {
            return Err(CommandError::from(WorkspaceError::InvalidPath(
                "cannot move an entry into itself".into(),
            )));
        }
        if target.starts_with(&source) {
            return Err(CommandError::from(WorkspaceError::InvalidPath(
                "cannot move an entry into its own subtree".into(),
            )));
        }
        if fs::symlink_metadata(&target).is_ok() {
            return Err(CommandError::from(WorkspaceError::AlreadyExists));
        }
        fs::rename(&source, &target)
            .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?;
        refresh_cache(&store, &root).map_err(CommandError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

// ── Code 工作台（M-CM1）：模组项目文件夹的通用文件浏览/读写 ──
//
// root = studio_config.mod_root 下的项目文件夹（模组列表「开发」按钮进入）。
// 安全防护与 Markdown 工作区同源：`validate_relative_path` 拒绝越界分量，
// `existing_path` 逐级校验符号链接并 canonicalize 归属，写入走 `write_atomic`。

/// 项目 root 解析：modRoot（存在、非符号链接、canonicalize）→ 项目相对路径
/// （同一套逐级防护）且必须是目录。
fn code_root(store: &Store, project: &str) -> Result<PathBuf, WorkspaceError> {
    let config = store.studio_config()?;
    let Some(mod_root) = config.mod_root else {
        return Err(WorkspaceError::ModRootNotConfigured);
    };
    let metadata = fs::symlink_metadata(&mod_root).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            WorkspaceError::RootUnavailable
        } else {
            WorkspaceError::Io(error)
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(WorkspaceError::Symlink);
    }
    if !metadata.is_dir() {
        return Err(WorkspaceError::NotDirectory);
    }
    let canonical_root = fs::canonicalize(mod_root)?;
    let project_relative = validate_relative_path(project, false)?;
    let project_dir = existing_path(&canonical_root, &project_relative)?;
    if !project_dir.is_dir() {
        return Err(WorkspaceError::NotFound);
    }
    Ok(project_dir)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeProjectRequest {
    /// 项目文件夹相对 modRoot 的路径（mod_projects.rel_path）。
    pub project: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodePathRequest {
    pub project: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeTreeNode {
    pub name: String,
    pub relative_path: String,
    pub kind: &'static str,
    /// 文件字节大小；folder 为 None。
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub children: Vec<CodeTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeTreeResponse {
    pub root_path: String,
    pub file_count: usize,
    pub folder_count: usize,
    /// true = 条目超出预算或深度上限，树被截断。
    pub truncated: bool,
    pub entries: Vec<CodeTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeTextDocument {
    pub relative_path: String,
    pub content: String,
    pub size: usize,
    pub revision: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeWriteTextRequest {
    pub project: String,
    pub relative_path: String,
    pub content: String,
    /// 乐观锁：与当前内容的 sha256 不一致即拒绝（None = 允许新建）。
    #[serde(default)]
    pub expected_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodePackageTypeInfo {
    pub type_id: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodePackageInfo {
    pub relative_path: String,
    /// 磁盘文件大小。
    pub size: u64,
    pub entry_count: usize,
    /// 条目解压尺寸合计（DBPF index 视角）。
    pub decompressed_total: u64,
    /// 类型直方图（按数量降序，取前 12）。
    pub types: Vec<CodePackageTypeInfo>,
}

#[derive(Default)]
struct CodeScanBudget {
    remaining: usize,
    truncated: bool,
}

fn scan_code_dir(
    root: &Path,
    current: &Path,
    relative: &str,
    depth: usize,
    budget: &mut CodeScanBudget,
) -> Result<Vec<CodeTreeNode>, WorkspaceError> {
    let mut children = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    children.sort_by_key(|entry| entry.file_name());
    let mut nodes = Vec::new();
    for child in children {
        if budget.remaining == 0 {
            budget.truncated = true;
            break;
        }
        let path = child.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let name = child.file_name().to_string_lossy().into_owned();
        let child_relative = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        budget.remaining -= 1;
        if metadata.is_dir() {
            let canonical = fs::canonicalize(&path)?;
            if !canonical.starts_with(root) {
                continue;
            }
            let grandchildren = if depth >= MAX_CODE_TREE_DEPTH {
                budget.truncated = true;
                Vec::new()
            } else {
                scan_code_dir(root, &canonical, &child_relative, depth + 1, budget)?
            };
            nodes.push(CodeTreeNode {
                name,
                relative_path: child_relative,
                kind: "folder",
                size: None,
                children: grandchildren,
            });
        } else if metadata.is_file() {
            nodes.push(CodeTreeNode {
                name,
                relative_path: child_relative,
                kind: "file",
                size: Some(metadata.len()),
                children: Vec::new(),
            });
        }
    }
    // 目录优先、组内按名排序（sort_by_key 稳定，保持既有名序）。
    nodes.sort_by_key(|node| (node.kind != "folder", node.name.clone()));
    Ok(nodes)
}

fn count_code_entries(nodes: &[CodeTreeNode]) -> (usize, usize) {
    let mut files = 0;
    let mut folders = 0;
    for node in nodes {
        if node.kind == "folder" {
            folders += 1;
            let (f, d) = count_code_entries(&node.children);
            files += f;
            folders += d;
        } else {
            files += 1;
        }
    }
    (files, folders)
}

#[command]
pub async fn code_tree(
    state: State<'_, AppState>,
    request: CodeProjectRequest,
) -> Result<CodeTreeResponse, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let root = code_root(&store, &request.project).map_err(CommandError::from)?;
        let mut budget = CodeScanBudget {
            remaining: MAX_CODE_TREE_ENTRIES,
            truncated: false,
        };
        let entries =
            scan_code_dir(&root, &root, "", 0, &mut budget).map_err(CommandError::from)?;
        let (file_count, folder_count) = count_code_entries(&entries);
        Ok(CodeTreeResponse {
            root_path: root.to_string_lossy().into_owned(),
            file_count,
            folder_count,
            truncated: budget.truncated,
            entries,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[command]
pub async fn code_read_text(
    state: State<'_, AppState>,
    request: CodePathRequest,
) -> Result<CodeTextDocument, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let root = code_root(&store, &request.project).map_err(CommandError::from)?;
        let relative = validate_relative_path(&request.relative_path, false)
            .map_err(CommandError::from)?;
        let path = existing_path(&root, &relative).map_err(CommandError::from)?;
        if !path.is_file() {
            return Err(CommandError::from(WorkspaceError::NotFile));
        }
        let metadata = fs::metadata(&path)
            .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?;
        if metadata.len() > MAX_CODE_TEXT_BYTES as u64 {
            return Err(CommandError::from(WorkspaceError::TooLarge(
                MAX_CODE_TEXT_BYTES,
            )));
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize + 1);
        File::open(&path)
            .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?
            .take((MAX_CODE_TEXT_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?;
        if bytes.len() > MAX_CODE_TEXT_BYTES {
            return Err(CommandError::from(WorkspaceError::TooLarge(
                MAX_CODE_TEXT_BYTES,
            )));
        }
        let content = String::from_utf8(bytes)
            .map_err(|error| CommandError::from(WorkspaceError::InvalidUtf8(error)))?;
        let document_revision = revision(content.as_bytes());
        Ok(CodeTextDocument {
            relative_path: request.relative_path,
            content,
            size: metadata.len() as usize,
            revision: document_revision,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[command]
pub async fn code_write_text(
    state: State<'_, AppState>,
    request: CodeWriteTextRequest,
) -> Result<CodeTextDocument, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let root = code_root(&store, &request.project).map_err(CommandError::from)?;
        let relative = validate_relative_path(&request.relative_path, false)
            .map_err(CommandError::from)?;
        let parent = existing_parent(&root, &relative).map_err(CommandError::from)?;
        let target = parent.join(
            relative
                .file_name()
                .ok_or_else(|| CommandError::from(WorkspaceError::NotFound))?,
        );
        let existing = match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(CommandError::from(WorkspaceError::Symlink));
            }
            Ok(metadata) if !metadata.is_file() => {
                return Err(CommandError::from(WorkspaceError::NotFile));
            }
            Ok(_) => {
                let mut bytes = Vec::new();
                File::open(&target)
                    .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?
                    .read_to_end(&mut bytes)
                    .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?;
                Some(revision(&bytes))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(CommandError::from(WorkspaceError::Io(error))),
        };
        if let Some(actual) = &existing {
            if request.expected_revision.as_deref() != Some(actual.as_str()) {
                return Err(CommandError::from(WorkspaceError::RevisionConflict));
            }
        }
        let bytes = request.content.as_bytes();
        if bytes.len() > MAX_CODE_TEXT_BYTES {
            return Err(CommandError::from(WorkspaceError::TooLarge(
                MAX_CODE_TEXT_BYTES,
            )));
        }
        write_atomic(&target, bytes, existing.is_some()).map_err(CommandError::from)?;
        Ok(CodeTextDocument {
            relative_path: request.relative_path,
            content: request.content.clone(),
            size: bytes.len(),
            revision: revision(bytes),
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

/// DBPF 容器独立打开统计（不进 PackageManager）：条目数、解压总量、类型直方图。
fn code_package_stats(
    path: &Path,
) -> Result<(usize, u64, Vec<CodePackageTypeInfo>), WorkspaceError> {
    let package = dbpf::Package::open(path)
        .map_err(|error| WorkspaceError::InvalidPath(error.to_string()))?;
    let mut histogram: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut decompressed_total = 0u64;
    for entry in package.entries() {
        *histogram.entry(entry.id.type_id).or_insert(0) += 1;
        decompressed_total += u64::from(entry.decompressed_size);
    }
    let mut types: Vec<CodePackageTypeInfo> = histogram
        .into_iter()
        .map(|(type_id, count)| CodePackageTypeInfo { type_id, count })
        .collect();
    types.sort_by(|a, b| b.count.cmp(&a.count).then(a.type_id.cmp(&b.type_id)));
    types.truncate(12);
    Ok((package.entries().len(), decompressed_total, types))
}

#[command]
pub async fn code_package_info(
    state: State<'_, AppState>,
    request: CodePathRequest,
) -> Result<CodePackageInfo, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let root = code_root(&store, &request.project).map_err(CommandError::from)?;
        let relative = validate_relative_path(&request.relative_path, false)
            .map_err(CommandError::from)?;
        let path = existing_path(&root, &relative).map_err(CommandError::from)?;
        if !path.is_file() {
            return Err(CommandError::from(WorkspaceError::NotFile));
        }
        let size = fs::metadata(&path)
            .map_err(|error| CommandError::from(WorkspaceError::Io(error)))?
            .len();
        let (entry_count, decompressed_total, types) =
            code_package_stats(&path).map_err(|error| match error {
                WorkspaceError::Io(error) => {
                    CommandError::from(WorkspaceError::InvalidPath(error.to_string()))
                }
                other => CommandError::from(other),
            })?;
        Ok(CodePackageInfo {
            relative_path: request.relative_path,
            size,
            entry_count,
            decompressed_total,
            types,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal_and_non_markdown_paths() {
        assert!(validate_relative_path("../escape", false).is_err());
        assert!(validate_relative_path(r"mods\escape", false).is_err());
        assert!(markdown_path("mods/readme.txt").is_err());
        assert!(markdown_path("mods/README.md").is_ok());
    }

    #[test]
    fn revisions_are_stable_sha256() {
        assert_eq!(
            revision(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn rename_names_must_be_single_components() {
        assert!(validate_relative_path("ok-name", false).is_ok());
        assert_eq!(Path::new("single").components().count(), 1);
        assert!(Path::new("nested/name").components().count() > 1);
    }

    #[test]
    fn move_into_own_subtree_is_detected() {
        let root = std::env::temp_dir().join(format!("openscp-move-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let source = root.join("mods");
        fs::create_dir_all(source.join("inner")).unwrap();
        let moved_to = source.join("inner");
        let target = moved_to.join("mods");
        assert!(target.starts_with(&source));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn scan_lists_folders_and_markdown_files() {
        let raw = std::env::temp_dir().join(format!("openscp-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raw);
        fs::create_dir_all(raw.join("mods/inner")).unwrap();
        fs::write(raw.join("mods/notes.md"), b"# notes").unwrap();
        fs::write(raw.join("root.md"), b"root").unwrap();
        fs::write(raw.join("mods/ignore.txt"), b"x").unwrap();
        let root = fs::canonicalize(&raw).unwrap();
        let mut entries = Vec::new();
        scan_entries(&root, &root, "", &mut entries).unwrap();
        let found: Vec<_> = entries
            .iter()
            .map(|entry| (entry.relative_path.as_str(), entry.kind))
            .collect();
        assert!(found.contains(&("mods", "folder")));
        assert!(found.contains(&("mods/inner", "folder")));
        assert!(found.contains(&("mods/notes.md", "file")));
        assert!(found.contains(&("root.md", "file")));
        assert!(!found.iter().any(|(path, _)| *path == "mods/ignore.txt"));
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn root_level_documents_resolve_to_root_parent() {
        let raw = std::env::temp_dir().join(format!("openscp-parent-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raw);
        fs::create_dir_all(&raw).unwrap();
        let root = fs::canonicalize(&raw).unwrap();
        assert_eq!(existing_parent(&root, Path::new("root.md")).unwrap(), root);
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn code_tree_lists_folders_first_with_sizes() {
        let raw = std::env::temp_dir().join(format!("openscp-code-tree-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raw);
        fs::create_dir_all(raw.join("Mod/SimCityData")).unwrap();
        fs::write(raw.join("Mod/readme.txt"), b"hello").unwrap();
        fs::write(raw.join("Mod/SimCityData/a.package"), vec![0u8; 10]).unwrap();
        let root = fs::canonicalize(&raw).unwrap();
        let mut budget = CodeScanBudget {
            remaining: MAX_CODE_TREE_ENTRIES,
            truncated: false,
        };
        let entries = scan_code_dir(&root, &root, "", 0, &mut budget).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, "folder");
        let children = &entries[0].children;
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].kind, "folder");
        assert_eq!(children[0].relative_path, "Mod/SimCityData");
        assert_eq!(children[1].kind, "file");
        assert_eq!(children[1].size, Some(5));
        assert!(!budget.truncated);
        let (files, folders) = count_code_entries(&entries);
        assert_eq!((files, folders), (2, 2));
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn code_tree_budget_truncates_and_reports() {
        let raw = std::env::temp_dir().join(format!("openscp-code-cap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&raw);
        fs::create_dir_all(&raw).unwrap();
        for index in 0..5 {
            fs::write(raw.join(format!("f{index}.txt")), b"x").unwrap();
        }
        let root = fs::canonicalize(&raw).unwrap();
        let mut budget = CodeScanBudget {
            remaining: 3,
            truncated: false,
        };
        let entries = scan_code_dir(&root, &root, "", 0, &mut budget).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(budget.truncated);
        let _ = fs::remove_dir_all(&raw);
    }

    #[test]
    fn code_package_stats_counts_entries_by_type() {
        use dbpf::{OverlayEntry, ResourceId, write_uncompressed_overlay};
        let path =
            std::env::temp_dir().join(format!("openscp-code-pkg-{}.package", std::process::id()));
        let entries = [
            OverlayEntry::new(
                ResourceId {
                    type_id: 0x00B1_B104,
                    group: 0,
                    instance: 1,
                },
                b"abcd",
            ),
            OverlayEntry::new(
                ResourceId {
                    type_id: 0x00B1_B104,
                    group: 0,
                    instance: 2,
                },
                b"efgh",
            ),
            OverlayEntry::new(
                ResourceId {
                    type_id: 0x2F4E_681C,
                    group: 0,
                    instance: 3,
                },
                b"ijkl",
            ),
        ];
        fs::write(&path, write_uncompressed_overlay(&entries).unwrap()).unwrap();
        let (count, total, types) = code_package_stats(&path).unwrap();
        assert_eq!(count, 3);
        assert_eq!(total, 12);
        assert_eq!(types[0].type_id, 0x00B1_B104);
        assert_eq!(types[0].count, 2);
        assert_eq!(types[1].type_id, 0x2F4E_681C);
        fs::remove_file(&path).unwrap();
    }
}
