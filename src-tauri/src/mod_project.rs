//! Mod 项目管理服务：项目管理目录、项目 CRUD、分组与统计。
//!
//! 项目 = 项目管理根目录（studio_config.mod_root）下的一个子文件夹 +
//! `mod_projects` 表记录。删除项目仅删除记录并保留磁盘文件夹；
//! 改名同步磁盘文件夹（rel_path 跟随 name）。
//!
//! `setup_status` 聚合三处关键路径（文档工作区 / 游戏目录 / 项目管理目录）
//! 的配置状态：启动缺失检测、首次引导、设置页地址管理共用这一个命令。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sc_store::{ModProject, ModProjectInput, Store, StoreError};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::activity::{AppState, CommandError};

/// 项目名的禁用字符（Windows 文件夹名非法字符 + 路径分隔符）。
const FORBIDDEN_NAME_CHARS: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
const PROJECT_NAME_MAX: usize = 80;
/// 项目状态白名单。
const PROJECT_STATUSES: &[&str] = &["active", "released", "archived"];
/// 磁盘占用统计的目录深度与文件数上限（防失控遍历）。
const DISK_SCAN_MAX_DEPTH: usize = 6;
const DISK_SCAN_MAX_FILES: usize = 50_000;
/// 近 N 天活动直方。
const ACTIVITY_WINDOW_DAYS: usize = 30;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, thiserror::Error)]
enum ModProjectError {
    #[error("path is empty or contains a NUL byte")]
    InvalidPath,
    #[error("path does not exist")]
    NotFound,
    #[error("path is not a directory")]
    NotDirectory,
    #[error("path is a symbolic link or reparse point")]
    Symlink,
    #[error("mod project root is not configured")]
    RootNotConfigured,
    #[error("project name is empty, too long, or contains forbidden characters")]
    InvalidName,
    #[error("status must be one of active/released/archived")]
    InvalidStatus,
    #[error("a project folder named {0} already exists")]
    ProjectExists(String),
    #[error("a group named {0} already exists")]
    GroupExists(String),
    #[error("group {0} was not found")]
    GroupNotFound(i64),
    #[error("mod project io error: {0}")]
    Io(#[from] io::Error),
    #[error("mod project store error: {0}")]
    Store(#[from] StoreError),
}

impl ModProjectError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidPath => "invalid_path",
            Self::NotFound => "not_found",
            Self::NotDirectory => "invalid_type",
            Self::Symlink => "path_forbidden",
            Self::RootNotConfigured => "not_configured",
            Self::InvalidName => "invalid_name",
            Self::InvalidStatus => "invalid_status",
            Self::ProjectExists(_) | Self::GroupExists(_) => "already_exists",
            Self::GroupNotFound(_) => "not_found",
            Self::Io(_) | Self::Store(_) => "mod_project_error",
        }
    }
}

impl From<ModProjectError> for CommandError {
    fn from(error: ModProjectError) -> Self {
        CommandError::new(error.code(), error.to_string())
    }
}

// ── 目录状态（setup_status 与设置页共用） ──

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPathStatus {
    path: Option<String>,
    exists: bool,
    is_directory: bool,
}

fn path_status(path: Option<String>) -> SetupPathStatus {
    let Some(path) = path else {
        return SetupPathStatus {
            path: None,
            exists: false,
            is_directory: false,
        };
    };
    match fs::symlink_metadata(&path) {
        Ok(metadata) => SetupPathStatus {
            path: Some(path),
            exists: true,
            is_directory: metadata.is_dir(),
        },
        Err(_) => SetupPathStatus {
            path: Some(path),
            exists: false,
            is_directory: false,
        },
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupStatusResponse {
    /// 文档工作区根目录。
    workspace: SetupPathStatus,
    /// 游戏数据目录（SimCityData）。
    game: SetupPathStatus,
    /// 项目管理根目录。
    mod_root: SetupPathStatus,
    onboarding_completed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModRootRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioConfigStatus {
    pub mod_root: Option<String>,
    pub onboarding_completed: bool,
}

// ── 校验 ──

fn validate_directory(path: &str) -> Result<PathBuf, ModProjectError> {
    if path.is_empty() || path.contains('\0') {
        return Err(ModProjectError::InvalidPath);
    }
    let path = PathBuf::from(path);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            ModProjectError::NotFound
        } else {
            ModProjectError::Io(error)
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(ModProjectError::Symlink);
    }
    if !metadata.is_dir() {
        return Err(ModProjectError::NotDirectory);
    }
    Ok(path)
}

/// 项目名即文件夹名：去首尾空白后需非空、无禁用字符、非保留名。
fn validate_project_name(name: &str) -> Result<String, ModProjectError> {
    let name = name.trim();
    if name.is_empty() || name.len() > PROJECT_NAME_MAX || name.starts_with('.') {
        return Err(ModProjectError::InvalidName);
    }
    if name
        .chars()
        .any(|c| FORBIDDEN_NAME_CHARS.contains(&c) || c.is_control())
    {
        return Err(ModProjectError::InvalidName);
    }
    Ok(name.to_owned())
}

fn validate_status(status: &str) -> Result<(), ModProjectError> {
    if PROJECT_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(ModProjectError::InvalidStatus)
    }
}

fn mod_root_or_error(store: &Store) -> Result<PathBuf, ModProjectError> {
    let config = store.studio_config().map_err(ModProjectError::from)?;
    let Some(root) = config.mod_root else {
        return Err(ModProjectError::RootNotConfigured);
    };
    validate_directory(&root)
}

fn project_view(mod_root: Option<&Path>, project: ModProject) -> ModProjectView {
    let folder_path = mod_root.map(|root| root.join(&project.rel_path));
    let folder_exists = folder_path.as_deref().is_some_and(|path| path.is_dir());
    ModProjectView {
        id: project.id,
        name: project.name,
        rel_path: project.rel_path,
        group_id: project.group_id,
        description: project.description,
        status: project.status,
        created_at: project.created_at,
        updated_at: project.updated_at,
        folder_path: folder_path.map(|path| path.to_string_lossy().into_owned()),
        folder_exists,
    }
}

// ── 命令：配置状态与 studio config ──

#[tauri::command]
pub async fn setup_status(state: State<'_, AppState>) -> Result<SetupStatusResponse, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let workspace = store
            .workspace_config()
            .map_err(ModProjectError::from)?
            .map(|config| config.root_path);
        let game = store
            .app_settings()
            .map_err(ModProjectError::from)?
            .and_then(|settings| settings.game_data_path);
        let config = store.studio_config().map_err(ModProjectError::from)?;
        Ok(SetupStatusResponse {
            workspace: path_status(workspace),
            game: path_status(game),
            mod_root: path_status(config.mod_root),
            onboarding_completed: config.onboarding_completed,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn studio_set_mod_root(
    state: State<'_, AppState>,
    request: ModRootRequest,
) -> Result<StudioConfigStatus, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let canonical = validate_directory(&request.path)?;
        let config = store
            .set_studio_config(Some(&canonical.to_string_lossy()), false)
            .map_err(ModProjectError::from)?;
        Ok(StudioConfigStatus {
            mod_root: config.mod_root,
            onboarding_completed: config.onboarding_completed,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn studio_complete_onboarding(
    state: State<'_, AppState>,
) -> Result<StudioConfigStatus, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let config = store
            .set_studio_config(None, true)
            .map_err(ModProjectError::from)?;
        Ok(StudioConfigStatus {
            mod_root: config.mod_root,
            onboarding_completed: config.onboarding_completed,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

// ── 命令：项目 CRUD ──

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProjectView {
    pub id: i64,
    pub name: String,
    pub rel_path: String,
    pub group_id: Option<i64>,
    pub description: Option<String>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    /// 项目文件夹绝对路径；mod_root 未配置时为 None。
    pub folder_path: Option<String>,
    /// 磁盘文件夹是否仍存在（用户手动删除后为 false）。
    pub folder_exists: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProjectCreateRequest {
    pub name: String,
    pub group_id: Option<i64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProjectUpdateRequest {
    pub id: i64,
    pub name: String,
    pub group_id: Option<i64>,
    pub description: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProjectIdRequest {
    pub id: i64,
}

#[tauri::command]
pub async fn mod_project_list(
    state: State<'_, AppState>,
) -> Result<Vec<ModProjectView>, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let config = store.studio_config().map_err(ModProjectError::from)?;
        let mod_root = config.mod_root.as_deref().map(PathBuf::from);
        let projects = store.mod_project_list().map_err(ModProjectError::from)?;
        Ok::<Vec<ModProjectView>, ModProjectError>(
            projects
                .into_iter()
                .map(|project| project_view(mod_root.as_deref(), project))
                .collect(),
        )
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

#[tauri::command]
pub async fn mod_project_create(
    state: State<'_, AppState>,
    request: ModProjectCreateRequest,
) -> Result<ModProjectView, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let name = validate_project_name(&request.name)?;
        let mod_root = mod_root_or_error(&store)?;
        if let Some(group_id) = request.group_id {
            let exists = store
                .mod_group_list()
                .map_err(ModProjectError::from)?
                .iter()
                .any(|group| group.id == group_id);
            if !exists {
                return Err(ModProjectError::GroupNotFound(group_id));
            }
        }
        let folder = mod_root.join(&name);
        if folder.exists() {
            return Err(ModProjectError::ProjectExists(name));
        }
        fs::create_dir(&folder)?;
        let input = ModProjectInput {
            name: name.clone(),
            rel_path: name,
            group_id: request.group_id,
            description: request.description,
            status: "active".into(),
        };
        let project = match store.mod_project_create(&input) {
            Ok(project) => project,
            // 建库失败回滚磁盘文件夹，避免留下无记录的孤儿目录
            Err(error) => {
                let _ = fs::remove_dir(&folder);
                return Err(ModProjectError::from(error));
            }
        };
        Ok(project_view(Some(&mod_root), project))
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

#[tauri::command]
pub async fn mod_project_update(
    state: State<'_, AppState>,
    request: ModProjectUpdateRequest,
) -> Result<ModProjectView, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let name = validate_project_name(&request.name)?;
        validate_status(&request.status)?;
        let project = store
            .mod_project_get(request.id)
            .map_err(ModProjectError::from)?;
        if let Some(group_id) = request.group_id {
            let exists = store
                .mod_group_list()
                .map_err(ModProjectError::from)?
                .iter()
                .any(|group| group.id == group_id);
            if !exists {
                return Err(ModProjectError::GroupNotFound(group_id));
            }
        }
        // 改名 → 磁盘文件夹同步改名（rel_path 跟随）
        if name != project.name {
            let mod_root = mod_root_or_error(&store)?;
            let from = mod_root.join(&project.rel_path);
            let to_rel = name.clone();
            let to = mod_root.join(&to_rel);
            if !from.exists() {
                return Err(ModProjectError::NotFound);
            }
            if to.exists() {
                return Err(ModProjectError::ProjectExists(name));
            }
            fs::rename(&from, &to)?;
            if let Err(error) = store.mod_project_rename(request.id, &name, &to_rel) {
                let _ = fs::rename(&to, &from);
                return Err(ModProjectError::from(error));
            }
        }
        store
            .mod_project_set_group(request.id, request.group_id)
            .map_err(ModProjectError::from)?;
        store
            .mod_project_set_description(request.id, request.description.as_deref())
            .map_err(ModProjectError::from)?;
        let updated = store
            .mod_project_set_status(request.id, &request.status)
            .map_err(ModProjectError::from)?;
        let config = store.studio_config().map_err(ModProjectError::from)?;
        Ok(project_view(
            config.mod_root.as_deref().map(PathBuf::from).as_deref(),
            updated,
        ))
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

#[tauri::command]
pub async fn mod_project_delete(
    state: State<'_, AppState>,
    request: ModProjectIdRequest,
) -> Result<(), CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        store
            .mod_project_delete(request.id)
            .map_err(ModProjectError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

// ── 命令：分组 ──

#[tauri::command]
pub async fn mod_group_list(
    state: State<'_, AppState>,
) -> Result<Vec<sc_store::ModProjectGroup>, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        store.mod_group_list().map_err(ModProjectError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModGroupNameRequest {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModGroupIdRequest {
    pub id: i64,
}

#[tauri::command]
pub async fn mod_group_create(
    state: State<'_, AppState>,
    request: ModGroupNameRequest,
) -> Result<sc_store::ModProjectGroup, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let name = validate_project_name(&request.name)?;
        let existing = store.mod_group_list().map_err(ModProjectError::from)?;
        if existing.iter().any(|group| group.name == name) {
            return Err(ModProjectError::GroupExists(name));
        }
        let next_sort = existing.iter().map(|group| group.sort).max().unwrap_or(0) + 1;
        store
            .mod_group_create(&name, next_sort)
            .map_err(ModProjectError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModGroupRenameRequest {
    pub id: i64,
    pub name: String,
}

#[tauri::command]
pub async fn mod_group_rename(
    state: State<'_, AppState>,
    request: ModGroupRenameRequest,
) -> Result<sc_store::ModProjectGroup, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let name = validate_project_name(&request.name)?;
        let existing = store.mod_group_list().map_err(ModProjectError::from)?;
        if existing
            .iter()
            .any(|group| group.name == name && group.id != request.id)
        {
            return Err(ModProjectError::GroupExists(name));
        }
        store
            .mod_group_rename(request.id, &name)
            .map_err(ModProjectError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

#[tauri::command]
pub async fn mod_group_delete(
    state: State<'_, AppState>,
    request: ModGroupIdRequest,
) -> Result<(), CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        store
            .mod_group_delete(request.id)
            .map_err(ModProjectError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

// ── 命令：统计 ──

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModGroupCount {
    pub group_id: Option<i64>,
    pub name: Option<String>,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModStatusCount {
    pub status: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDailyCount {
    /// 当日零点的 epoch 毫秒（UTC 日界，前端按本地时区渲染）。
    pub day_start_ms: i64,
    pub created: u64,
    pub updated: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModProjectStatsResponse {
    pub total: u64,
    pub disk_bytes: u64,
    pub group_counts: Vec<ModGroupCount>,
    pub status_counts: Vec<ModStatusCount>,
    pub recent_activity: Vec<ModDailyCount>,
}

fn disk_usage(path: &Path, depth: usize, budget: &mut usize) -> u64 {
    if depth > DISK_SCAN_MAX_DEPTH || *budget == 0 {
        return 0;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            total += disk_usage(&entry.path(), depth + 1, budget);
        } else {
            total += metadata.len();
        }
    }
    total
}

#[tauri::command]
pub async fn mod_project_stats(
    state: State<'_, AppState>,
) -> Result<ModProjectStatsResponse, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let aggregates = store
            .mod_project_aggregates()
            .map_err(ModProjectError::from)?;
        let groups = store.mod_group_list().map_err(ModProjectError::from)?;
        let group_counts = aggregates
            .group_counts
            .into_iter()
            .map(|(group_id, count)| {
                let name = group_id.and_then(|id| {
                    groups
                        .iter()
                        .find(|group| group.id == id)
                        .map(|group| group.name.clone())
                });
                ModGroupCount {
                    group_id,
                    name,
                    count: count as u64,
                }
            })
            .collect();
        let status_counts = aggregates
            .status_counts
            .into_iter()
            .map(|(status, count)| ModStatusCount {
                status,
                count: count as u64,
            })
            .collect();
        // 近 30 天直方：按 UTC 日分桶（含空白天），前端按本地时区渲染标签
        let today = (now_day_ms() / DAY_MS) as usize;
        let mut recent: Vec<ModDailyCount> = (0..ACTIVITY_WINDOW_DAYS)
            .rev()
            .map(|offset| ModDailyCount {
                day_start_ms: (today - offset) as i64 * DAY_MS,
                created: 0,
                updated: 0,
            })
            .collect();
        for (created_at, updated_at) in &aggregates.timestamps {
            for timestamp in [created_at, updated_at] {
                let day = (timestamp / DAY_MS) as usize;
                if day > today {
                    continue;
                }
                let offset = today - day;
                if offset < ACTIVITY_WINDOW_DAYS {
                    if let Some(bucket) = recent.get_mut(ACTIVITY_WINDOW_DAYS - 1 - offset) {
                        if timestamp == created_at {
                            bucket.created += 1;
                        }
                        if timestamp == updated_at {
                            bucket.updated += 1;
                        }
                    }
                }
            }
        }
        let disk_bytes = store
            .studio_config()
            .ok()
            .and_then(|config| config.mod_root)
            .map(|root| {
                let mut budget = DISK_SCAN_MAX_FILES;
                disk_usage(Path::new(&root), 0, &mut budget)
            })
            .unwrap_or(0);
        Ok(ModProjectStatsResponse {
            total: aggregates.total,
            disk_bytes,
            group_counts,
            status_counts,
            recent_activity: recent,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

fn now_day_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64 / DAY_MS * DAY_MS)
        .unwrap_or(0)
}
