use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const CURRENT_SCHEMA_VERSION: i32 = 6;
pub const DEFAULT_LIST_LIMIT: usize = 100;
pub const MAX_LIST_LIMIT: usize = 1_000;

/// 单个资源负载上限（32 MiB）：超过即拒绝入版本库。
pub const BLOB_MAX_BYTES: usize = 32 * 1024 * 1024;
/// 版本库总占用上限（512 MiB）：超出后按最旧优先回收无引用 blob。
pub const BLOB_STORE_MAX_BYTES: i64 = 512 * 1024 * 1024;
/// 渲染遥测行数上限（滚动窗口）。
pub const RENDER_TELEMETRY_MAX_ROWS: i64 = 10_000;
/// 渲染遥测保留时长（30 天）。
pub const RENDER_TELEMETRY_MAX_AGE_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// 每 N 次遥测写入触发一次裁剪，避免每插一行都做全表扫描。
pub const TELEMETRY_PRUNE_INTERVAL: u64 = 256;

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("store io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("store sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("store JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported store schema version {0}")]
    UnsupportedSchemaVersion(i32),
    #[error("operation {0} was not found")]
    OperationNotFound(i64),
    #[error("changeset {0} was not found")]
    ChangesetNotFound(i64),
    #[error("mod project {0} was not found")]
    ModProjectNotFound(i64),
    #[error("mod project group {0} was not found")]
    ModGroupNotFound(i64),
    #[error("resource payload is {0} bytes, over the {BLOB_MAX_BYTES} byte limit")]
    BlobTooLarge(usize),
    #[error("version blob store is full ({0} bytes referenced)")]
    BlobStoreFull(i64),
}

#[derive(Debug)]
pub struct Store {
    connection: Mutex<Connection>,
    /// 遥测写入计数，用于按间隔触发裁剪。
    telemetry_writes: AtomicU64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: i64,
    pub kind: String,
    pub target: String,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub bytes_in: Option<i64>,
    pub bytes_out: Option<i64>,
    pub detail: Option<Value>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: i64,
    pub operation_id: Option<i64>,
    pub level: String,
    pub topic: String,
    pub message: String,
    pub payload: Option<Value>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub id: i64,
    pub path: String,
    pub size: i64,
    pub entry_count: i64,
    pub version: i64,
    pub open_count: i64,
    pub last_opened_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventInput {
    pub operation_id: Option<i64>,
    pub level: String,
    pub topic: String,
    pub message: String,
    pub payload: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PackageInput {
    pub path: String,
    pub size: i64,
    pub entry_count: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub game_data_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConfig {
    pub root_path: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 工作台全局配置（单行）：模组开发根目录 + 引导完成标志。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StudioConfig {
    pub mod_root: Option<String>,
    pub onboarding_completed: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 项目分组（模组开发）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModProjectGroup {
    pub id: i64,
    pub name: String,
    pub sort: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 模组项目：磁盘上是项目管理根下的一个子文件夹，
/// 记录删除时保留磁盘文件夹（防误删由前端确认兜底）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModProject {
    pub id: i64,
    pub name: String,
    pub rel_path: String,
    pub group_id: Option<i64>,
    pub description: Option<String>,
    /// active | released | archived
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModProjectInput {
    pub name: String,
    pub rel_path: String,
    pub group_id: Option<i64>,
    pub description: Option<String>,
    pub status: String,
}

/// 项目统计聚合：分组计数 / 状态计数 / 创建与更新时间戳（前端做 30 天直方）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModProjectAggregates {
    /// (group_id, count)；group_id = None 表示未分组。
    pub group_counts: Vec<(Option<i64>, i64)>,
    /// (status, count)
    pub status_counts: Vec<(String, i64)>,
    /// (created_at, updated_at)
    pub timestamps: Vec<(i64, i64)>,
    /// 项目总数。
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FolderCacheEntry {
    pub relative_path: String,
    pub readme_relative_path: Option<String>,
    pub last_seen_at: i64,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActivityClearResult {
    pub operations: usize,
    pub events: usize,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        migrate(&connection)?;
        let store = Self {
            connection: Mutex::new(connection),
            telemetry_writes: AtomicU64::new(0),
        };
        store.prune_render_telemetry()?;
        Ok(store)
    }

    pub fn start_operation(&self, kind: &str, target: &str, detail: Option<&Value>) -> Result<i64> {
        let detail = detail.map(serde_json::to_string).transpose()?;
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection.execute(
            "INSERT INTO operations (kind, target, status, detail, created_at) VALUES (?1, ?2, 'running', ?3, ?4)",
            params![kind, target, detail, now_millis()],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn finish_operation(
        &self,
        id: i64,
        status: &str,
        duration_ms: i64,
        bytes_in: Option<i64>,
        bytes_out: Option<i64>,
        detail: Option<&Value>,
    ) -> Result<()> {
        let detail = detail.map(serde_json::to_string).transpose()?;
        let connection = self.connection.lock().expect("store mutex poisoned");
        let changed = connection.execute(
            "UPDATE operations SET status = ?1, duration_ms = ?2, bytes_in = ?3, bytes_out = ?4, detail = COALESCE(?5, detail), finished_at = ?6 WHERE id = ?7",
            params![status, duration_ms, bytes_in, bytes_out, detail, now_millis(), id],
        )?;
        if changed == 0 {
            return Err(StoreError::OperationNotFound(id));
        }
        Ok(())
    }

    pub fn append_event(&self, input: &EventInput) -> Result<Event> {
        let payload = input
            .payload
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let created_at = now_millis();
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection.execute(
            "INSERT INTO events (operation_id, level, topic, message, payload, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![input.operation_id, input.level, input.topic, input.message, payload, created_at],
        )?;
        let id = connection.last_insert_rowid();
        Ok(Event {
            id,
            operation_id: input.operation_id,
            level: input.level.clone(),
            topic: input.topic.clone(),
            message: input.message.clone(),
            payload: input.payload.clone(),
            created_at,
        })
    }

    pub fn record_package_open(&self, input: &PackageInput) -> Result<Package> {
        let opened_at = now_millis();
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection.execute(
            "INSERT INTO packages (path, size, entry_count, version, open_count, last_opened_at) VALUES (?1, ?2, ?3, ?4, 1, ?5)
             ON CONFLICT(path) DO UPDATE SET size = excluded.size, entry_count = excluded.entry_count, version = excluded.version, open_count = packages.open_count + 1, last_opened_at = excluded.last_opened_at",
            params![input.path, input.size, input.entry_count, input.version, opened_at],
        )?;
        connection
            .query_row(
                "SELECT id, path, size, entry_count, version, open_count, last_opened_at FROM packages WHERE path = ?1",
                params![input.path],
                package_from_row,
            )
            .map_err(StoreError::from)
    }

    pub fn list_operations(&self, limit: usize) -> Result<Vec<Operation>> {
        let limit = bounded_limit(limit);
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, kind, target, status, duration_ms, bytes_in, bytes_out, detail, created_at, finished_at FROM operations ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], operation_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn list_events(&self, limit: usize) -> Result<Vec<Event>> {
        let limit = bounded_limit(limit);
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, operation_id, level, topic, message, payload, created_at FROM events ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], event_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn list_packages(&self, limit: usize) -> Result<Vec<Package>> {
        let limit = bounded_limit(limit);
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, path, size, entry_count, version, open_count, last_opened_at FROM packages ORDER BY last_opened_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], package_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn app_settings(&self) -> Result<Option<AppSettings>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection
            .query_row(
                "SELECT game_data_path, created_at, updated_at FROM app_settings WHERE id = 1",
                [],
                |row| {
                    Ok(AppSettings {
                        game_data_path: row.get(0)?,
                        created_at: row.get(1)?,
                        updated_at: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn set_game_data_path(&self, game_data_path: Option<&str>) -> Result<AppSettings> {
        let now = now_millis();
        let connection = self.connection.lock().expect("store mutex poisoned");
        let created_at: Option<i64> = connection
            .query_row(
                "SELECT created_at FROM app_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let created_at = created_at.unwrap_or(now);
        connection.execute(
            "INSERT INTO app_settings (id, game_data_path, created_at, updated_at) VALUES (1, ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET game_data_path = excluded.game_data_path, updated_at = excluded.updated_at",
            params![game_data_path, created_at, now],
        )?;
        Ok(AppSettings {
            game_data_path: game_data_path.map(str::to_owned),
            created_at,
            updated_at: now,
        })
    }
    pub fn workspace_config(&self) -> Result<Option<WorkspaceConfig>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection
            .query_row(
                "SELECT root_path, created_at, updated_at FROM workspace_config WHERE id = 1",
                [],
                |row| {
                    Ok(WorkspaceConfig {
                        root_path: row.get(0)?,
                        created_at: row.get(1)?,
                        updated_at: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn set_workspace_config(&self, root_path: &str) -> Result<WorkspaceConfig> {
        let now = now_millis();
        let connection = self.connection.lock().expect("store mutex poisoned");
        let existing: Option<i64> = connection
            .query_row(
                "SELECT created_at FROM workspace_config WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let created_at = existing.unwrap_or(now);
        connection.execute(
            "INSERT INTO workspace_config (id, root_path, created_at, updated_at) VALUES (1, ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET root_path = excluded.root_path, updated_at = excluded.updated_at",
            params![root_path, created_at, now],
        )?;
        connection.execute("DELETE FROM workspace_folder_cache", [])?;
        Ok(WorkspaceConfig {
            root_path: root_path.into(),
            created_at,
            updated_at: now,
        })
    }

    pub fn replace_folder_cache(&self, entries: &[FolderCacheEntry]) -> Result<()> {
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM workspace_folder_cache", [])?;
        for entry in entries {
            transaction.execute(
                "INSERT INTO workspace_folder_cache (relative_path, readme_relative_path, last_seen_at) VALUES (?1, ?2, ?3)",
                params![entry.relative_path, entry.readme_relative_path, entry.last_seen_at],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn list_folder_cache(&self) -> Result<Vec<FolderCacheEntry>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT relative_path, readme_relative_path, last_seen_at FROM workspace_folder_cache ORDER BY relative_path",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(FolderCacheEntry {
                relative_path: row.get(0)?,
                readme_relative_path: row.get(1)?,
                last_seen_at: row.get(2)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }
    pub fn clear_activity(&self) -> Result<ActivityClearResult> {
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        let events = transaction.execute("DELETE FROM events", [])?;
        let operations = transaction.execute("DELETE FROM operations", [])?;
        transaction.commit()?;
        Ok(ActivityClearResult { operations, events })
    }

    /// 读工作台全局配置；行不存在时返回默认值（未配置/未完成引导）。
    pub fn studio_config(&self) -> Result<StudioConfig> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let config = connection
            .query_row(
                "SELECT mod_root, onboarding_completed, created_at, updated_at FROM studio_config WHERE id = 1",
                [],
                |row| {
                    Ok(StudioConfig {
                        mod_root: row.get(0)?,
                        onboarding_completed: row.get::<_, i64>(1)? != 0,
                        created_at: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)?
            .unwrap_or(StudioConfig {
                mod_root: None,
                onboarding_completed: false,
                created_at: 0,
                updated_at: 0,
            });
        Ok(config)
    }

    pub fn set_studio_config(
        &self,
        mod_root: Option<&str>,
        onboarding_completed: bool,
    ) -> Result<StudioConfig> {
        let now = now_millis();
        let connection = self.connection.lock().expect("store mutex poisoned");
        let existing: Option<(Option<String>, i64, i64)> = connection
            .query_row(
                "SELECT mod_root, onboarding_completed, created_at FROM studio_config WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        // 未传字段沿用现值：set_mod_root 不应顺手清掉引导标志，反之亦然。
        let (current_root, current_onboarding, created_at) = match existing {
            Some((root, flag, created_at)) => (root, flag != 0, created_at),
            None => (None, false, now),
        };
        let mod_root = mod_root.map_or(current_root.as_deref(), Some).map(str::to_owned);
        let onboarding = if onboarding_completed { true } else { current_onboarding };
        connection.execute(
            "INSERT INTO studio_config (id, mod_root, onboarding_completed, created_at, updated_at) VALUES (1, ?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET mod_root = excluded.mod_root, onboarding_completed = excluded.onboarding_completed, updated_at = excluded.updated_at",
            params![mod_root, onboarding as i64, created_at, now],
        )?;
        Ok(StudioConfig {
            mod_root,
            onboarding_completed: onboarding,
            created_at,
            updated_at: now,
        })
    }

    pub fn mod_group_list(&self) -> Result<Vec<ModProjectGroup>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, name, sort, created_at, updated_at FROM mod_project_groups ORDER BY sort, id",
        )?;
        let rows = statement.query_map([], mod_group_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn mod_group_create(&self, name: &str, sort: i64) -> Result<ModProjectGroup> {
        let now = now_millis();
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection.execute(
            "INSERT INTO mod_project_groups (name, sort, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            params![name, sort, now],
        )?;
        Ok(ModProjectGroup {
            id: connection.last_insert_rowid(),
            name: name.to_owned(),
            sort,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn mod_group_rename(&self, id: i64, name: &str) -> Result<ModProjectGroup> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let changed = connection.execute(
            "UPDATE mod_project_groups SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![name, now_millis(), id],
        )?;
        if changed == 0 {
            return Err(StoreError::ModGroupNotFound(id));
        }
        connection
            .query_row(
                "SELECT id, name, sort, created_at, updated_at FROM mod_project_groups WHERE id = ?1",
                params![id],
                mod_group_from_row,
            )
            .map_err(StoreError::from)
    }

    pub fn mod_group_delete(&self, id: i64) -> Result<()> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        // 组下项目经外键 ON DELETE SET NULL 自动回到未分组。
        let changed = connection.execute("DELETE FROM mod_project_groups WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(StoreError::ModGroupNotFound(id));
        }
        Ok(())
    }

    pub fn mod_project_list(&self) -> Result<Vec<ModProject>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, name, rel_path, group_id, description, status, created_at, updated_at
             FROM mod_projects ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], mod_project_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn mod_project_create(&self, input: &ModProjectInput) -> Result<ModProject> {
        let now = now_millis();
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection.execute(
            "INSERT INTO mod_projects (name, rel_path, group_id, description, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            params![
                input.name,
                input.rel_path,
                input.group_id,
                input.description,
                input.status,
                now
            ],
        )?;
        Ok(ModProject {
            id: connection.last_insert_rowid(),
            name: input.name.clone(),
            rel_path: input.rel_path.clone(),
            group_id: input.group_id,
            description: input.description.clone(),
            status: input.status.clone(),
            created_at: now,
            updated_at: now,
        })
    }

    /// 项目改名：name 与 rel_path（磁盘文件夹名）同步更新。
    pub fn mod_project_rename(&self, id: i64, name: &str, rel_path: &str) -> Result<ModProject> {
        Self::mod_project_update(&self.connection, id, |project| {
            project.name = name.to_owned();
            project.rel_path = rel_path.to_owned();
        })?;
        self.mod_project_get(id)
    }

    pub fn mod_project_set_group(&self, id: i64, group_id: Option<i64>) -> Result<ModProject> {
        Self::mod_project_update(&self.connection, id, |project| {
            project.group_id = group_id;
        })?;
        self.mod_project_get(id)
    }

    pub fn mod_project_set_status(&self, id: i64, status: &str) -> Result<ModProject> {
        Self::mod_project_update(&self.connection, id, |project| {
            project.status = status.to_owned();
        })?;
        self.mod_project_get(id)
    }

    pub fn mod_project_set_description(
        &self,
        id: i64,
        description: Option<&str>,
    ) -> Result<ModProject> {
        Self::mod_project_update(&self.connection, id, |project| {
            project.description = description.map(str::to_owned);
        })?;
        self.mod_project_get(id)
    }

    pub fn mod_project_delete(&self, id: i64) -> Result<()> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let changed = connection.execute("DELETE FROM mod_projects WHERE id = ?1", [id])?;
        if changed == 0 {
            return Err(StoreError::ModProjectNotFound(id));
        }
        Ok(())
    }

    pub fn mod_project_get(&self, id: i64) -> Result<ModProject> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection
            .query_row(
                "SELECT id, name, rel_path, group_id, description, status, created_at, updated_at
                 FROM mod_projects WHERE id = ?1",
                params![id],
                mod_project_from_row,
            )
            .optional()
            .map_err(StoreError::from)?
            .ok_or(StoreError::ModProjectNotFound(id))
    }

    pub fn mod_project_aggregates(&self) -> Result<ModProjectAggregates> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut group_counts = {
            let mut statement = connection.prepare(
                "SELECT group_id, COUNT(*) FROM mod_projects GROUP BY group_id",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        // 未分组固定在列（可能为 0），前端图表不用特判空序列。
        let total: i64 = group_counts.iter().map(|(_, count)| *count).sum();
        let ungrouped: i64 = group_counts
            .iter()
            .filter(|(id, _)| id.is_none())
            .map(|(_, count)| *count)
            .sum();
        if ungrouped == 0 {
            group_counts.push((None, 0));
        }
        let mut status_counts = {
            let mut statement = connection
                .prepare("SELECT status, COUNT(*) FROM mod_projects GROUP BY status ORDER BY status")?;
            let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        if status_counts.is_empty() {
            status_counts.push(("active".into(), 0));
        }
        let timestamps = {
            let mut statement = connection
                .prepare("SELECT created_at, updated_at FROM mod_projects")?;
            let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(ModProjectAggregates {
            group_counts,
            status_counts,
            timestamps,
            total: total as u64,
        })
    }

    /// 单字段更新助手：改字段 + 刷 updated_at；目标不存在报 NotFound。
    fn mod_project_update(
        connection: &Mutex<Connection>,
        id: i64,
        apply: impl FnOnce(&mut ModProject),
    ) -> Result<()> {
        let mut project = {
            let connection = connection.lock().expect("store mutex poisoned");
            connection
                .query_row(
                    "SELECT id, name, rel_path, group_id, description, status, created_at, updated_at
                     FROM mod_projects WHERE id = ?1",
                    params![id],
                    mod_project_from_row,
                )
                .optional()
                .map_err(StoreError::from)?
                .ok_or(StoreError::ModProjectNotFound(id))?
        };
        apply(&mut project);
        let connection = connection.lock().expect("store mutex poisoned");
        let changed = connection.execute(
            "UPDATE mod_projects SET name = ?1, rel_path = ?2, group_id = ?3, description = ?4, status = ?5, updated_at = ?6 WHERE id = ?7",
            params![
                project.name,
                project.rel_path,
                project.group_id,
                project.description,
                project.status,
                now_millis(),
                id
            ],
        )?;
        if changed == 0 {
            return Err(StoreError::ModProjectNotFound(id));
        }
        Ok(())
    }
}

fn migrate(connection: &Connection) -> Result<()> {
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    let mut version: i32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(StoreError::UnsupportedSchemaVersion(version));
    }
    if version == 0 {
        connection.execute_batch(
            "BEGIN;
             CREATE TABLE operations (
                 id INTEGER PRIMARY KEY,
                 kind TEXT NOT NULL,
                 target TEXT NOT NULL,
                 status TEXT NOT NULL,
                 duration_ms INTEGER,
                 bytes_in INTEGER,
                 bytes_out INTEGER,
                 detail TEXT,
                 created_at INTEGER NOT NULL,
                 finished_at INTEGER
             );
             CREATE TABLE events (
                 id INTEGER PRIMARY KEY,
                 operation_id INTEGER REFERENCES operations(id) ON DELETE SET NULL,
                 level TEXT NOT NULL,
                 topic TEXT NOT NULL,
                 message TEXT NOT NULL,
                 payload TEXT,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE packages (
                 id INTEGER PRIMARY KEY,
                 path TEXT NOT NULL UNIQUE,
                 size INTEGER NOT NULL,
                 entry_count INTEGER NOT NULL,
                 version INTEGER NOT NULL,
                 open_count INTEGER NOT NULL DEFAULT 1,
                 last_opened_at INTEGER NOT NULL
             );
             CREATE INDEX events_created_at_idx ON events(created_at DESC);
             CREATE INDEX events_operation_id_idx ON events(operation_id);
             CREATE INDEX operations_created_at_idx ON operations(created_at DESC);
             CREATE INDEX operations_status_idx ON operations(status);
             CREATE INDEX packages_last_opened_at_idx ON packages(last_opened_at DESC);
             CREATE TABLE workspace_config (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 root_path TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE workspace_folder_cache (
                 relative_path TEXT PRIMARY KEY,
                 readme_relative_path TEXT,
                 last_seen_at INTEGER NOT NULL
             );
             CREATE TABLE app_settings (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 game_data_path TEXT,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             PRAGMA user_version = 3;
             COMMIT;",
        )?;
        // 基础库等价于 v3。**不要提前 return**：让后面的块在同一次 open 里继续级联，
        // 否则全新安装首次启动只到 v3，v4/v5 要第二次打开才建（旧实现的缺陷）。
        version = 3;
    }
    if version == 1 {
        connection.execute_batch(
            "BEGIN;
             CREATE TABLE workspace_config (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 root_path TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE workspace_folder_cache (
                 relative_path TEXT PRIMARY KEY,
                 readme_relative_path TEXT,
                 last_seen_at INTEGER NOT NULL
             );
             PRAGMA user_version = 2;
             COMMIT;",
        )?;
        version = 2;
    }
    if version <= 2 {
        connection.execute_batch(
            "BEGIN;
             CREATE TABLE app_settings (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 game_data_path TEXT,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             PRAGMA user_version = 3;
             COMMIT;",
        )?;
        version = 3;
    }
    if version <= 3 {
        connection.execute_batch(
            "BEGIN;
             CREATE TABLE resource_annotations (
                 id INTEGER PRIMARY KEY,
                 package_path TEXT NOT NULL,
                 type_id INTEGER NOT NULL,
                 group_id INTEGER NOT NULL,
                 instance INTEGER NOT NULL,
                 topic TEXT NOT NULL,
                 title TEXT NOT NULL,
                 content TEXT NOT NULL DEFAULT '',
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE INDEX resource_annotations_tgi_idx
                 ON resource_annotations(type_id, group_id, instance);
             CREATE INDEX resource_annotations_topic_idx
                 ON resource_annotations(topic);
             PRAGMA user_version = 4;
             COMMIT;",
        )?;
        version = 4;
    }
    if version <= 4 {
        connection.execute_batch(
            "BEGIN;
             CREATE TABLE changesets (
                 id INTEGER PRIMARY KEY,
                 target_path TEXT NOT NULL,
                 writer TEXT NOT NULL,
                 label TEXT,
                 note TEXT,
                 revision INTEGER NOT NULL,
                 resource_count INTEGER NOT NULL,
                 bytes_before INTEGER NOT NULL DEFAULT 0,
                 bytes_after INTEGER NOT NULL DEFAULT 0,
                 file_digest TEXT,
                 restored_from INTEGER,
                 detail TEXT,
                 created_at INTEGER NOT NULL
             );
             CREATE UNIQUE INDEX changesets_target_revision_idx
                 ON changesets(target_path, revision);
             CREATE INDEX changesets_target_created_idx
                 ON changesets(target_path, created_at DESC);
             CREATE INDEX changesets_created_at_idx ON changesets(created_at DESC);
             CREATE TABLE resource_blobs (
                 digest TEXT PRIMARY KEY,
                 bytes BLOB NOT NULL,
                 size INTEGER NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX resource_blobs_created_at_idx ON resource_blobs(created_at);
             CREATE TABLE change_items (
                 id INTEGER PRIMARY KEY,
                 changeset_id INTEGER NOT NULL REFERENCES changesets(id) ON DELETE CASCADE,
                 type_id INTEGER NOT NULL,
                 group_id INTEGER NOT NULL,
                 instance INTEGER NOT NULL,
                 before_digest TEXT REFERENCES resource_blobs(digest),
                 after_digest TEXT REFERENCES resource_blobs(digest),
                 before_size INTEGER NOT NULL DEFAULT 0,
                 after_size INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX change_items_changeset_idx ON change_items(changeset_id);
             CREATE INDEX change_items_tgi_idx ON change_items(type_id, group_id, instance);
             CREATE TABLE render_telemetry (
                 id INTEGER PRIMARY KEY,
                 session_key TEXT NOT NULL,
                 stage TEXT NOT NULL,
                 trigger TEXT NOT NULL,
                 duration_ms REAL NOT NULL,
                 metadata TEXT,
                 created_at INTEGER NOT NULL
             );
             CREATE INDEX render_telemetry_stage_created_idx
                 ON render_telemetry(stage, created_at DESC);
             CREATE INDEX render_telemetry_created_at_idx
                 ON render_telemetry(created_at DESC);
             CREATE INDEX render_telemetry_session_idx ON render_telemetry(session_key);
             PRAGMA user_version = 5;
             COMMIT;",
        )?;
    }
    if version <= 5 {
        connection.execute_batch(
            "BEGIN;
             CREATE TABLE mod_project_groups (
                 id INTEGER PRIMARY KEY,
                 name TEXT NOT NULL UNIQUE,
                 sort INTEGER NOT NULL DEFAULT 0,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE mod_projects (
                 id INTEGER PRIMARY KEY,
                 name TEXT NOT NULL,
                 rel_path TEXT NOT NULL UNIQUE,
                 group_id INTEGER REFERENCES mod_project_groups(id) ON DELETE SET NULL,
                 description TEXT,
                 status TEXT NOT NULL DEFAULT 'active',
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE INDEX mod_projects_group_idx ON mod_projects(group_id);
             CREATE INDEX mod_projects_updated_at_idx ON mod_projects(updated_at DESC);
             CREATE TABLE studio_config (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 mod_root TEXT,
                 onboarding_completed INTEGER NOT NULL DEFAULT 0,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             PRAGMA user_version = 6;
             COMMIT;",
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceAnnotation {
    pub id: i64,
    pub package_path: String,
    pub type_id: u32,
    pub group_id: u32,
    pub instance: u32,
    pub topic: String,
    pub title: String,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResourceAnnotationInput {
    pub package_path: String,
    pub type_id: u32,
    pub group_id: u32,
    pub instance: u32,
    pub topic: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationTopicStat {
    pub topic: String,
    pub annotation_count: i64,
    pub resource_count: i64,
}

// ---------- 编辑版本管理 ----------

/// 一条版本记录（同一 `target_path` 上一个单调递增的 `revision` = 「版本 N」）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangesetSummary {
    pub id: i64,
    pub target_path: String,
    pub writer: String,
    pub label: Option<String>,
    pub note: Option<String>,
    pub revision: i64,
    pub resource_count: i64,
    pub bytes_before: i64,
    pub bytes_after: i64,
    /// 写出后整文件 sha256（仅元数据，用于外部漂移检测）。
    pub file_digest: Option<String>,
    /// 回滚产生时指向源 revision。
    pub restored_from: Option<i64>,
    pub detail: Option<Value>,
    pub created_at: i64,
}

/// 写入一条 changeset 时，单个资源的前后字节。`None` 分别表示「新增」与「删除」。
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeItemInput {
    pub type_id: u32,
    pub group_id: u32,
    pub instance: u32,
    pub before: Option<Vec<u8>>,
    pub after: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChangesetInput {
    pub target_path: String,
    /// `'locale_overlay'` | `'property_overlay'` | `'baseline'` | `'rollback'`
    pub writer: String,
    pub label: Option<String>,
    pub note: Option<String>,
    pub file_digest: Option<String>,
    pub restored_from: Option<i64>,
    pub detail: Option<Value>,
    pub items: Vec<ChangeItemInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangeItemRecord {
    pub type_id: u32,
    pub group_id: u32,
    pub instance: u32,
    pub before_digest: Option<String>,
    pub after_digest: Option<String>,
    pub before_size: i64,
    pub after_size: i64,
}

/// 某 target 在某个 revision 上的一个资源（已解出字节，供回滚重建）。
#[derive(Debug, Clone, PartialEq)]
pub struct VersionResource {
    pub type_id: u32,
    pub group_id: u32,
    pub instance: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VersionTargetSummary {
    pub target_path: String,
    pub latest_revision: i64,
    pub changeset_count: i64,
    pub last_written_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VersionDeleteResult {
    pub changesets: usize,
    pub items: usize,
    pub blobs_reclaimed: usize,
}

// ---------- 渲染遥测 ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderTelemetryInput {
    pub session_key: String,
    pub stage: String,
    pub trigger: String,
    pub duration_ms: f64,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderStageStat {
    pub stage: String,
    pub count: i64,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub last_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenderTelemetrySummary {
    pub window_days: i64,
    pub since: i64,
    pub total_count: i64,
    pub stages: Vec<RenderStageStat>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryClearResult {
    pub rows: usize,
}

fn row_to_annotation(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResourceAnnotation> {
    Ok(ResourceAnnotation {
        id: row.get(0)?,
        package_path: row.get(1)?,
        type_id: row.get::<_, i64>(2)? as u32,
        group_id: row.get::<_, i64>(3)? as u32,
        instance: row.get::<_, i64>(4)? as u32,
        topic: row.get(5)?,
        title: row.get(6)?,
        content: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

impl Store {
    pub fn create_annotation(&self, input: &ResourceAnnotationInput) -> Result<ResourceAnnotation> {
        let now = now_millis();
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection.execute(
            "INSERT INTO resource_annotations
                 (package_path, type_id, group_id, instance, topic, title, content,
                  created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                input.package_path,
                input.type_id as i64,
                input.group_id as i64,
                input.instance as i64,
                input.topic.trim(),
                input.title.trim(),
                input.content,
                now
            ],
        )?;
        let id = connection.last_insert_rowid();
        Ok(ResourceAnnotation {
            id,
            package_path: input.package_path.clone(),
            type_id: input.type_id,
            group_id: input.group_id,
            instance: input.instance,
            topic: input.topic.trim().to_string(),
            title: input.title.trim().to_string(),
            content: input.content.clone(),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update_annotation(
        &self,
        id: i64,
        topic: &str,
        title: &str,
        content: &str,
    ) -> Result<ResourceAnnotation> {
        let now = now_millis();
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let changed = connection.execute(
            "UPDATE resource_annotations
             SET topic = ?2, title = ?3, content = ?4, updated_at = ?5
             WHERE id = ?1",
            params![id, topic.trim(), title.trim(), content, now],
        )?;
        if changed == 0 {
            return Err(StoreError::OperationNotFound(id));
        }
        drop(changed);
        Ok(Self::annotation_by_id(&connection, id)?)
    }

    pub fn delete_annotation(&self, id: i64) -> Result<()> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let changed = connection.execute(
            "DELETE FROM resource_annotations WHERE id = ?1",
            params![id],
        )?;
        if changed == 0 {
            return Err(StoreError::OperationNotFound(id));
        }
        Ok(())
    }

    fn annotation_by_id(connection: &Connection, id: i64) -> Result<ResourceAnnotation> {
        connection
            .query_row(
                "SELECT id, package_path, type_id, group_id, instance, topic, title,
                        content, created_at, updated_at
                 FROM resource_annotations WHERE id = ?1",
                params![id],
                row_to_annotation,
            )
            .map_err(Into::into)
    }

    pub fn list_annotations_for_tgi(
        &self,
        type_id: u32,
        group_id: u32,
        instance: u32,
    ) -> Result<Vec<ResourceAnnotation>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, package_path, type_id, group_id, instance, topic, title,
                    content, created_at, updated_at
             FROM resource_annotations
             WHERE type_id = ?1 AND group_id = ?2 AND instance = ?3
             ORDER BY updated_at DESC",
        )?;
        let rows = statement
            .query_map(
                params![type_id as i64, group_id as i64, instance as i64],
                row_to_annotation,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn list_annotations(&self, limit: usize) -> Result<Vec<ResourceAnnotation>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, package_path, type_id, group_id, instance, topic, title,
                    content, created_at, updated_at
             FROM resource_annotations ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let rows = statement
            .query_map(params![bounded_limit(limit) as i64], row_to_annotation)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn annotation_topic_stats(&self) -> Result<Vec<AnnotationTopicStat>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT topic,
                    COUNT(*) AS annotation_count,
                    COUNT(DISTINCT type_id || ':' || group_id || ':' || instance) AS resource_count
             FROM resource_annotations GROUP BY topic ORDER BY annotation_count DESC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(AnnotationTopicStat {
                    topic: row.get(0)?,
                    annotation_count: row.get(1)?,
                    resource_count: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

impl Store {
    // ---------- 编辑版本管理 ----------

    /// 内容寻址写入：相同字节只存一份，返回 sha256 十六进制摘要。
    fn put_blob_conn(connection: &Connection, bytes: &[u8]) -> Result<String> {
        if bytes.len() > BLOB_MAX_BYTES {
            return Err(StoreError::BlobTooLarge(bytes.len()));
        }
        let digest = hex_digest(bytes);
        connection.execute(
            "INSERT OR IGNORE INTO resource_blobs (digest, bytes, size, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            params![digest, bytes, bytes.len() as i64, now_millis()],
        )?;
        Ok(digest)
    }

    pub fn load_blob(&self, digest: &str) -> Result<Option<Vec<u8>>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection
            .query_row(
                "SELECT bytes FROM resource_blobs WHERE digest = ?1",
                params![digest],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(StoreError::from)
    }

    /// 记录一条版本：单事务内取号（`MAX(revision)+1`）、落 blob、写 changesets + change_items。
    pub fn record_changeset(&self, input: &ChangesetInput) -> Result<ChangesetSummary> {
        struct Prepared {
            type_id: u32,
            group_id: u32,
            instance: u32,
            before: Option<String>,
            after: Option<String>,
            before_size: i64,
            after_size: i64,
        }

        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        let revision: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(revision), 0) + 1 FROM changesets WHERE target_path = ?1",
            params![input.target_path],
            |row| row.get(0),
        )?;
        let created_at = now_millis();
        let mut bytes_before = 0i64;
        let mut bytes_after = 0i64;
        let mut prepared = Vec::with_capacity(input.items.len());
        for item in &input.items {
            let before_size = item.before.as_ref().map(|b| b.len() as i64).unwrap_or(0);
            let after_size = item.after.as_ref().map(|b| b.len() as i64).unwrap_or(0);
            bytes_before += before_size;
            bytes_after += after_size;
            prepared.push(Prepared {
                type_id: item.type_id,
                group_id: item.group_id,
                instance: item.instance,
                before: item
                    .before
                    .as_deref()
                    .map(|bytes| Self::put_blob_conn(&transaction, bytes))
                    .transpose()?,
                after: item
                    .after
                    .as_deref()
                    .map(|bytes| Self::put_blob_conn(&transaction, bytes))
                    .transpose()?,
                before_size,
                after_size,
            });
        }
        let detail = input.detail.as_ref().map(serde_json::to_string).transpose()?;
        transaction.execute(
            "INSERT INTO changesets \
             (target_path, writer, label, note, revision, resource_count, bytes_before, bytes_after, \
              file_digest, restored_from, detail, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                input.target_path,
                input.writer,
                input.label,
                input.note,
                revision,
                prepared.len() as i64,
                bytes_before,
                bytes_after,
                input.file_digest,
                input.restored_from,
                detail,
                created_at
            ],
        )?;
        let changeset_id = transaction.last_insert_rowid();
        {
            let mut statement = transaction.prepare(
                "INSERT INTO change_items \
                 (changeset_id, type_id, group_id, instance, before_digest, after_digest, \
                  before_size, after_size) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            )?;
            for row in &prepared {
                statement.execute(params![
                    changeset_id,
                    row.type_id,
                    row.group_id,
                    row.instance,
                    row.before,
                    row.after,
                    row.before_size,
                    row.after_size
                ])?;
            }
        }
        transaction.commit()?;
        Ok(ChangesetSummary {
            id: changeset_id,
            target_path: input.target_path.clone(),
            writer: input.writer.clone(),
            label: input.label.clone(),
            note: input.note.clone(),
            revision,
            resource_count: prepared.len() as i64,
            bytes_before,
            bytes_after,
            file_digest: input.file_digest.clone(),
            restored_from: input.restored_from,
            detail: input.detail.clone(),
            created_at,
        })
    }

    pub fn list_version_targets(&self) -> Result<Vec<VersionTargetSummary>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT target_path, MAX(revision), COUNT(*), MAX(created_at) \
             FROM changesets GROUP BY target_path ORDER BY MAX(created_at) DESC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(VersionTargetSummary {
                    target_path: row.get(0)?,
                    latest_revision: row.get(1)?,
                    changeset_count: row.get(2)?,
                    last_written_at: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn list_changesets(
        &self,
        target_path: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ChangesetSummary>> {
        let limit = bounded_limit(limit) as i64;
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, target_path, writer, label, note, revision, resource_count, bytes_before, \
                    bytes_after, file_digest, restored_from, detail, created_at \
             FROM changesets \
             WHERE (?1 IS NULL OR target_path = ?1) \
             ORDER BY created_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = statement
            .query_map(params![target_path, limit], changeset_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn changeset_detail(
        &self,
        changeset_id: i64,
    ) -> Result<(ChangesetSummary, Vec<ChangeItemRecord>)> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let summary = connection
            .query_row(
                "SELECT id, target_path, writer, label, note, revision, resource_count, bytes_before, \
                        bytes_after, file_digest, restored_from, detail, created_at \
                 FROM changesets WHERE id = ?1",
                params![changeset_id],
                changeset_from_row,
            )
            .optional()?
            .ok_or(StoreError::ChangesetNotFound(changeset_id))?;
        let mut statement = connection.prepare(
            "SELECT type_id, group_id, instance, before_digest, after_digest, before_size, after_size \
             FROM change_items WHERE changeset_id = ?1 \
             ORDER BY type_id, group_id, instance",
        )?;
        let items = statement
            .query_map(params![changeset_id], |row| {
                Ok(ChangeItemRecord {
                    type_id: row.get(0)?,
                    group_id: row.get(1)?,
                    instance: row.get(2)?,
                    before_digest: row.get(3)?,
                    after_digest: row.get(4)?,
                    before_size: row.get(5)?,
                    after_size: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((summary, items))
    }

    /// 某 target 在 revision N 的资源集：每个 TGI 取 `revision <= N` 里最大的那条的 after；
    /// 该版本已被删除（after 为 NULL）的资源不返回。
    pub fn resource_states_at_revision(
        &self,
        target_path: &str,
        revision: i64,
    ) -> Result<Vec<VersionResource>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT ci.type_id, ci.group_id, ci.instance, b.bytes \
             FROM change_items ci \
             JOIN changesets c ON c.id = ci.changeset_id \
             JOIN resource_blobs b ON b.digest = ci.after_digest \
             WHERE c.target_path = ?1 \
               AND c.revision = ( \
                   SELECT MAX(c2.revision) FROM change_items ci2 \
                   JOIN changesets c2 ON c2.id = ci2.changeset_id \
                   WHERE c2.target_path = ?1 AND c2.revision <= ?2 \
                     AND ci2.type_id = ci.type_id \
                     AND ci2.group_id = ci.group_id \
                     AND ci2.instance = ci.instance) \
             ORDER BY ci.type_id, ci.group_id, ci.instance",
        )?;
        let rows = statement
            .query_map(params![target_path, revision], |row| {
                Ok(VersionResource {
                    type_id: row.get(0)?,
                    group_id: row.get(1)?,
                    instance: row.get(2)?,
                    bytes: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// 删除一条版本记录（只删元数据与 items，**绝不动磁盘文件**），随后回收无引用 blob。
    pub fn delete_changeset(&self, changeset_id: i64) -> Result<VersionDeleteResult> {
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        let exists: Option<i64> = transaction
            .query_row(
                "SELECT id FROM changesets WHERE id = ?1",
                params![changeset_id],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(StoreError::ChangesetNotFound(changeset_id));
        }
        let items = transaction.execute(
            "DELETE FROM change_items WHERE changeset_id = ?1",
            params![changeset_id],
        )?;
        transaction.execute("DELETE FROM changesets WHERE id = ?1", params![changeset_id])?;
        transaction.commit()?;
        let blobs_reclaimed = Self::gc_unreferenced_blobs(&connection)?;
        Ok(VersionDeleteResult {
            changesets: 1,
            items,
            blobs_reclaimed,
        })
    }

    /// 回收没有任何 change_items 引用的 blob。返回删除条数。
    fn gc_unreferenced_blobs(connection: &Connection) -> Result<usize> {
        let removed = connection.execute(
            "DELETE FROM resource_blobs WHERE digest NOT IN ( \
                 SELECT before_digest FROM change_items WHERE before_digest IS NOT NULL \
                 UNION SELECT after_digest FROM change_items WHERE after_digest IS NOT NULL)",
            [],
        )?;
        Ok(removed)
    }

    /// 该 target 最新版本记录的整文件摘要（外部漂移检测用）。
    pub fn latest_target_digest(&self, target_path: &str) -> Result<Option<String>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection
            .query_row(
                "SELECT file_digest FROM changesets WHERE target_path = ?1 \
                 ORDER BY revision DESC LIMIT 1",
                params![target_path],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map(|value| value.flatten())
            .map_err(StoreError::from)
    }

    // ---------- 渲染遥测 ----------

    /// 批量写入渲染遥测；按间隔触发滚动窗口裁剪。
    pub fn record_render_telemetry(&self, entries: &[RenderTelemetryInput]) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }
        let mut connection = self.connection.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        let created_at = now_millis();
        {
            let mut statement = transaction.prepare(
                "INSERT INTO render_telemetry \
                 (session_key, stage, trigger, duration_ms, metadata, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for entry in entries {
                let metadata = entry.metadata.as_ref().map(serde_json::to_string).transpose()?;
                statement.execute(params![
                    entry.session_key,
                    entry.stage,
                    entry.trigger,
                    entry.duration_ms,
                    metadata,
                    created_at
                ])?;
            }
        }
        transaction.commit()?;
        // 必须先释放连接锁再调用 prune（它自己会加锁）。
        drop(connection);
        let writes = self.telemetry_writes.fetch_add(1, Ordering::Relaxed) + 1;
        if writes % TELEMETRY_PRUNE_INTERVAL == 0 {
            self.prune_render_telemetry()?;
        }
        Ok(entries.len())
    }

    /// 按「保留时长 + 行数上限」裁剪遥测。`Store::open` 时也会调一次。
    pub fn prune_render_telemetry(&self) -> Result<usize> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let cutoff = now_millis() - RENDER_TELEMETRY_MAX_AGE_MS;
        let by_age = connection.execute(
            "DELETE FROM render_telemetry WHERE created_at < ?1",
            params![cutoff],
        )?;
        let by_count = connection.execute(
            "DELETE FROM render_telemetry WHERE id NOT IN ( \
                 SELECT id FROM render_telemetry ORDER BY id DESC LIMIT ?1)",
            params![RENDER_TELEMETRY_MAX_ROWS],
        )?;
        Ok(by_age + by_count)
    }

    pub fn clear_render_telemetry(&self) -> Result<TelemetryClearResult> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let rows = connection.execute("DELETE FROM render_telemetry", [])?;
        Ok(TelemetryClearResult { rows })
    }

    /// 按 stage 聚合窗口内的耗时；p50/p95 用每 stage 有界 500 行样本在 Rust 内算
    ///（SQLite 无原生分位数）。
    pub fn render_telemetry_summary(&self, window_days: i64) -> Result<RenderTelemetrySummary> {
        let window_days = window_days.clamp(1, 365);
        let since = now_millis() - window_days * 24 * 60 * 60 * 1000;
        let connection = self.connection.lock().expect("store mutex poisoned");
        let total_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM render_telemetry WHERE created_at >= ?1",
            params![since],
            |row| row.get(0),
        )?;
        let aggregates = {
            let mut statement = connection.prepare(
                "SELECT stage, COUNT(*), AVG(duration_ms), MIN(duration_ms), MAX(duration_ms) \
                 FROM render_telemetry WHERE created_at >= ?1 GROUP BY stage ORDER BY stage",
            )?;
            statement
                .query_map(params![since], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, f64>(2)?,
                        row.get::<_, f64>(3)?,
                        row.get::<_, f64>(4)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let mut sample_statement = connection.prepare(
            "SELECT duration_ms FROM render_telemetry \
             WHERE created_at >= ?1 AND stage = ?2 ORDER BY id DESC LIMIT 500",
        )?;
        let mut last_statement = connection.prepare(
            "SELECT duration_ms FROM render_telemetry \
             WHERE created_at >= ?1 AND stage = ?2 ORDER BY id DESC LIMIT 1",
        )?;
        let mut stages = Vec::with_capacity(aggregates.len());
        for (stage, count, avg_ms, min_ms, max_ms) in aggregates {
            let mut sample = sample_statement
                .query_map(params![since, stage], |row| row.get::<_, f64>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            sample.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let p50_ms = percentile(&sample, 0.50);
            let p95_ms = percentile(&sample, 0.95);
            let last_ms = last_statement
                .query_row(params![since, stage], |row| row.get::<_, f64>(0))
                .optional()?
                .unwrap_or(0.0);
            stages.push(RenderStageStat {
                stage,
                count,
                avg_ms,
                min_ms,
                max_ms,
                p50_ms,
                p95_ms,
                last_ms,
            });
        }
        Ok(RenderTelemetrySummary {
            window_days,
            since,
            total_count,
            stages,
        })
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// 最近秩（nearest-rank）分位数，`sorted` 必须已升序。
fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (q * (sorted.len() - 1) as f64).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn changeset_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChangesetSummary> {
    Ok(ChangesetSummary {
        id: row.get(0)?,
        target_path: row.get(1)?,
        writer: row.get(2)?,
        label: row.get(3)?,
        note: row.get(4)?,
        revision: row.get(5)?,
        resource_count: row.get(6)?,
        bytes_before: row.get(7)?,
        bytes_after: row.get(8)?,
        file_digest: row.get(9)?,
        restored_from: row.get(10)?,
        detail: parse_json(row.get(11)?)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?,
        created_at: row.get(12)?,
    })
}

fn bounded_limit(limit: usize) -> usize {
    limit.clamp(1, MAX_LIST_LIMIT)
}

fn mod_group_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModProjectGroup> {
    Ok(ModProjectGroup {
        id: row.get(0)?,
        name: row.get(1)?,
        sort: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

fn mod_project_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ModProject> {
    Ok(ModProject {
        id: row.get(0)?,
        name: row.get(1)?,
        rel_path: row.get(2)?,
        group_id: row.get(3)?,
        description: row.get(4)?,
        status: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_millis() as i64
}

fn operation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Operation> {
    Ok(Operation {
        id: row.get(0)?,
        kind: row.get(1)?,
        target: row.get(2)?,
        status: row.get(3)?,
        duration_ms: row.get(4)?,
        bytes_in: row.get(5)?,
        bytes_out: row.get(6)?,
        detail: parse_json(row.get(7)?)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?,
        created_at: row.get(8)?,
        finished_at: row.get(9)?,
    })
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    Ok(Event {
        id: row.get(0)?,
        operation_id: row.get(1)?,
        level: row.get(2)?,
        topic: row.get(3)?,
        message: row.get(4)?,
        payload: parse_json(row.get(5)?)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?,
        created_at: row.get(6)?,
    })
}

fn package_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Package> {
    Ok(Package {
        id: row.get(0)?,
        path: row.get(1)?,
        size: row.get(2)?,
        entry_count: row.get(3)?,
        version: row.get(4)?,
        open_count: row.get(5)?,
        last_opened_at: row.get(6)?,
    })
}

fn parse_json(value: Option<String>) -> Result<Option<Value>> {
    value
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(StoreError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn temp_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("openscp-store-{label}-{}.db", std::process::id()))
    }

    fn clean(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn migration_is_idempotent_and_sets_version() {
        let path = temp_path("migration");
        clean(&path);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.list_operations(10).unwrap(), Vec::<Operation>::new());
        drop(store);
        let store = Store::open(&path).unwrap();
        let connection = store.connection.lock().unwrap();
        let version: i32 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
        drop(connection);
        drop(store);
        clean(&path);
    }

    #[test]
    fn workspace_config_and_cache_are_persisted() {
        let path = temp_path("workspace");
        clean(&path);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.workspace_config().unwrap(), None);
        let config = store.set_workspace_config("C:/workspace").unwrap();
        assert_eq!(config.root_path, "C:/workspace");
        store
            .replace_folder_cache(&[FolderCacheEntry {
                relative_path: "mods/example".into(),
                readme_relative_path: Some("mods/example/README.md".into()),
                last_seen_at: config.updated_at,
            }])
            .unwrap();
        assert_eq!(store.list_folder_cache().unwrap().len(), 1);
        store.set_workspace_config("C:/other").unwrap();
        assert!(store.list_folder_cache().unwrap().is_empty());
        drop(store);
        clean(&path);
    }

    #[test]
    fn v1_database_migrates_to_v2() {
        let path = temp_path("v1");
        clean(&path);
        {
            let connection = rusqlite::Connection::open(&path).unwrap();
            connection
                .execute_batch("PRAGMA user_version = 1;")
                .unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.workspace_config().unwrap(), None);
        drop(store);
        clean(&path);
    }
    #[test]
    fn app_settings_are_persisted_and_overwritten() {
        let path = temp_path("settings");
        clean(&path);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.app_settings().unwrap(), None);
        let first = store.set_game_data_path(Some("C:/Games/SimCity")).unwrap();
        assert_eq!(first.game_data_path.as_deref(), Some("C:/Games/SimCity"));
        let second = store.set_game_data_path(Some("D:/Games/SimCity")).unwrap();
        assert_eq!(second.game_data_path.as_deref(), Some("D:/Games/SimCity"));
        assert_eq!(store.app_settings().unwrap(), Some(second));
        drop(store);
        clean(&path);
    }
    #[test]
    fn records_operation_event_and_package_history() {
        let path = temp_path("crud");
        clean(&path);
        let store = Store::open(&path).unwrap();
        let detail = serde_json::json!({"source": "test"});
        let operation = store
            .start_operation("package_open", "test.package", Some(&detail))
            .unwrap();
        store
            .finish_operation(operation, "success", 12, Some(20), Some(80), None)
            .unwrap();
        let event = store
            .append_event(&EventInput {
                operation_id: Some(operation),
                level: "info".into(),
                topic: "package".into(),
                message: "opened".into(),
                payload: Some(serde_json::json!({"entries": 2})),
            })
            .unwrap();
        assert_eq!(event.operation_id, Some(operation));
        let package = PackageInput {
            path: "test.package".into(),
            size: 20,
            entry_count: 2,
            version: 1,
        };
        assert_eq!(store.record_package_open(&package).unwrap().open_count, 1);
        assert_eq!(store.record_package_open(&package).unwrap().open_count, 2);
        assert_eq!(store.list_operations(10).unwrap()[0].status, "success");
        assert_eq!(
            store.list_events(10).unwrap()[0].payload,
            Some(serde_json::json!({"entries": 2}))
        );
        assert_eq!(store.list_packages(10).unwrap()[0].open_count, 2);
        let cleared = store.clear_activity().unwrap();
        assert_eq!(
            cleared,
            ActivityClearResult {
                operations: 1,
                events: 1
            }
        );
        assert!(store.list_operations(10).unwrap().is_empty());
        assert_eq!(store.list_packages(10).unwrap().len(), 1);
        drop(store);
        clean(&path);
    }

    #[test]
    fn concurrent_writes_are_serialized() {
        let path = temp_path("concurrent");
        clean(&path);
        let store = Arc::new(Store::open(&path).unwrap());
        let threads = (0..4)
            .map(|index| {
                let store = Arc::clone(&store);
                thread::spawn(move || {
                    store
                        .start_operation("scan", &index.to_string(), None)
                        .unwrap();
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(store.list_operations(10).unwrap().len(), 4);
        drop(store);
        clean(&path);
    }

    #[test]
    fn limits_are_bounded() {
        let path = temp_path("limit");
        clean(&path);
        let store = Store::open(&path).unwrap();
        for index in 0..3 {
            store
                .start_operation("scan", &index.to_string(), None)
                .unwrap();
        }
        assert!(store.list_operations(0).unwrap().len() <= 1);
        assert_eq!(store.list_operations(2).unwrap().len(), 2);
        assert!(store.list_operations(usize::MAX).unwrap().len() <= MAX_LIST_LIMIT);
        drop(store);
        clean(&path);
    }

    // ---------- v5: 编辑版本管理 ----------

    fn changeset_input(target: &str, items: Vec<ChangeItemInput>) -> ChangesetInput {
        ChangesetInput {
            target_path: target.into(),
            writer: "test".into(),
            label: None,
            note: None,
            file_digest: None,
            restored_from: None,
            detail: None,
            items,
        }
    }

    fn change_item(instance: u32, before: Option<&[u8]>, after: Option<&[u8]>) -> ChangeItemInput {
        ChangeItemInput {
            type_id: 0x00B1_B104,
            group_id: 0x0987_8A01,
            instance,
            before: before.map(<[u8]>::to_vec),
            after: after.map(<[u8]>::to_vec),
        }
    }

    /// 纯 Rust fold —— 作为 `resource_states_at_revision`（SQL 路径）的可读对照。
    fn fold_at(
        history: &[(i64, Vec<ChangeItemInput>)],
        revision: i64,
    ) -> std::collections::BTreeMap<(u32, u32, u32), Vec<u8>> {
        let mut state = std::collections::BTreeMap::new();
        for (rev, items) in history {
            if *rev > revision {
                continue;
            }
            for item in items {
                let key = (item.type_id, item.group_id, item.instance);
                match &item.after {
                    Some(bytes) => {
                        state.insert(key, bytes.clone());
                    }
                    None => {
                        state.remove(&key);
                    }
                }
            }
        }
        state
    }

    #[test]
    fn fresh_database_reaches_current_version_on_first_open() {
        // 守 migrate() 阶梯修复：旧实现首次打开只到 v3，v4/v5 要第二次 open 才建。
        let path = temp_path("fresh-version");
        clean(&path);
        let store = Store::open(&path).unwrap();
        let connection = store.connection.lock().unwrap();
        let version: i32 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, CURRENT_SCHEMA_VERSION);
        // v5 的表当场可用（旧实现这里会报 no such table）
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM changesets", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        drop(connection);
        drop(store);
        clean(&path);
    }

    #[test]
    fn v4_database_migrates_to_v5_keeping_annotations() {
        let path = temp_path("v4-to-v5");
        clean(&path);
        let annotation_id = {
            let store = Store::open(&path).unwrap();
            let created = store
                .create_annotation(&ResourceAnnotationInput {
                    package_path: "C:/p.package".into(),
                    type_id: 1,
                    group_id: 2,
                    instance: 3,
                    topic: "t".into(),
                    title: "T".into(),
                    content: "c".into(),
                })
                .unwrap();
            // 人为降级到 v4：删掉 v5/v6 的表并回写 user_version
            {
                let connection = store.connection.lock().unwrap();
                connection
                    .execute_batch(
                        "BEGIN;
                         DROP TABLE studio_config;
                         DROP TABLE mod_projects;
                         DROP TABLE mod_project_groups;
                         DROP TABLE render_telemetry;
                         DROP TABLE change_items;
                         DROP TABLE resource_blobs;
                         DROP TABLE changesets;
                         PRAGMA user_version = 4;
                         COMMIT;",
                    )
                    .unwrap();
            }
            created.id
        };
        let store = Store::open(&path).unwrap();
        {
            let connection = store.connection.lock().unwrap();
            let version: i32 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, CURRENT_SCHEMA_VERSION);
        }
        let annotations = store.list_annotations(100).unwrap();
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].id, annotation_id);
        assert_eq!(store.list_version_targets().unwrap(), Vec::new());
        drop(store);
        clean(&path);
    }

    #[test]
    fn v5_database_migrates_to_v6_keeping_changesets() {
        let path = temp_path("v5-to-v6");
        clean(&path);
        let target = {
            let store = Store::open(&path).unwrap();
            let recorded = store
                .record_changeset(&changeset_input(
                    "C:/a.package",
                    vec![change_item(1, None, Some(b"one"))],
                ))
                .unwrap();
            // 人为降级到 v5：删掉 v6 的表并回写 user_version
            {
                let connection = store.connection.lock().unwrap();
                connection
                    .execute_batch(
                        "BEGIN;
                         DROP TABLE studio_config;
                         DROP TABLE mod_projects;
                         DROP TABLE mod_project_groups;
                         PRAGMA user_version = 5;
                         COMMIT;",
                    )
                    .unwrap();
            }
            recorded.target_path.clone()
        };
        let store = Store::open(&path).unwrap();
        assert_eq!(target, "C:/a.package");
        assert_eq!(store.list_version_targets().unwrap().len(), 1);
        // v6 表当场可用
        let config = store.studio_config().unwrap();
        assert!(!config.onboarding_completed);
        assert_eq!(config.mod_root, None);
        drop(store);
        clean(&path);
    }

    #[test]
    fn studio_config_and_mod_projects_crud() {
        let path = temp_path("studio-crud");
        clean(&path);
        let store = Store::open(&path).unwrap();

        // 配置：字段独立更新且互不清除
        let root = store.set_studio_config(Some("D:/mods"), false).unwrap();
        assert_eq!(root.mod_root.as_deref(), Some("D:/mods"));
        assert!(!root.onboarding_completed);
        let done = store.set_studio_config(None, true).unwrap();
        assert_eq!(done.mod_root.as_deref(), Some("D:/mods"));
        assert!(done.onboarding_completed);

        // 分组
        let group = store.mod_group_create("交通", 0).unwrap();
        let group2 = store.mod_group_create("公共", 1).unwrap();
        assert_eq!(
            store
                .mod_group_list()
                .unwrap()
                .into_iter()
                .map(|g| g.name)
                .collect::<Vec<_>>(),
            vec!["交通".to_owned(), "公共".to_owned()]
        );

        // 项目 CRUD
        let created = store
            .mod_project_create(&ModProjectInput {
                name: "地铁Mod".into(),
                rel_path: "metro-mod".into(),
                group_id: Some(group.id),
                description: Some("磁悬浮当地铁".into()),
                status: "active".into(),
            })
            .unwrap();
        assert_eq!(created.status, "active");
        let renamed = store
            .mod_project_rename(created.id, "地铁Mod2", "metro-mod-2")
            .unwrap();
        assert_eq!(renamed.name, "地铁Mod2");
        assert_eq!(renamed.rel_path, "metro-mod-2");
        let moved = store.mod_project_set_group(created.id, Some(group2.id)).unwrap();
        assert_eq!(moved.group_id, Some(group2.id));
        store
            .mod_project_set_status(created.id, "released")
            .unwrap();
        let described = store
            .mod_project_set_description(created.id, None)
            .unwrap();
        assert_eq!(described.description, None);
        assert_eq!(store.mod_project_list().unwrap().len(), 1);

        // 删除分组 → 组下项目经外键回落；项目已改挂 group2 不受影响
        store.mod_group_delete(group.id).unwrap();
        assert!(matches!(
            store.mod_group_delete(group.id),
            Err(StoreError::ModGroupNotFound(_))
        ));
        assert_eq!(
            store.mod_project_get(created.id).unwrap().group_id,
            Some(group2.id)
        );

        store.mod_project_delete(created.id).unwrap();
        assert!(matches!(
            store.mod_project_delete(created.id),
            Err(StoreError::ModProjectNotFound(_))
        ));
        drop(store);
        clean(&path);
    }

    #[test]
    fn changeset_revision_is_monotonic_per_target() {
        let path = temp_path("changeset-revision");
        clean(&path);
        let store = Store::open(&path).unwrap();
        let first = store
            .record_changeset(&changeset_input("C:/a.package", vec![change_item(1, None, Some(b"one"))]))
            .unwrap();
        let second = store
            .record_changeset(&changeset_input(
                "C:/a.package",
                vec![change_item(1, Some(b"one"), Some(b"two"))],
            ))
            .unwrap();
        let other = store
            .record_changeset(&changeset_input("C:/b.package", vec![change_item(9, None, Some(b"x"))]))
            .unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(second.revision, 2);
        // 版本号按 target 各自独立
        assert_eq!(other.revision, 1);
        assert_eq!(first.bytes_before, 0);
        assert_eq!(first.bytes_after, 3);
        assert_eq!(second.bytes_before, 3);
        assert_eq!(second.bytes_after, 3);

        let targets = store.list_version_targets().unwrap();
        assert_eq!(targets.len(), 2);
        let a = targets.iter().find(|t| t.target_path == "C:/a.package").unwrap();
        assert_eq!(a.latest_revision, 2);
        assert_eq!(a.changeset_count, 2);

        let listed = store.list_changesets(Some("C:/a.package"), 100).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].revision, 2); // 倒序
        drop(store);
        clean(&path);
    }

    #[test]
    fn blob_is_deduplicated_across_changesets() {
        let path = temp_path("blob-dedup");
        clean(&path);
        let store = Store::open(&path).unwrap();
        let payload = vec![7u8; 4096];
        store
            .record_changeset(&changeset_input(
                "C:/a.package",
                vec![change_item(1, None, Some(&payload))],
            ))
            .unwrap();
        // 第二次写出同样的字节 -> 只应存在一份 blob
        store
            .record_changeset(&changeset_input(
                "C:/a.package",
                vec![change_item(2, None, Some(&payload))],
            ))
            .unwrap();
        let connection = store.connection.lock().unwrap();
        let blobs: i64 = connection
            .query_row("SELECT COUNT(*) FROM resource_blobs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(blobs, 1, "相同字节必须去重");
        drop(connection);
        drop(store);
        clean(&path);
    }

    #[test]
    fn delete_changeset_cascades_and_reclaims_blobs() {
        let path = temp_path("changeset-delete");
        clean(&path);
        let store = Store::open(&path).unwrap();
        let first = store
            .record_changeset(&changeset_input("C:/a.package", vec![change_item(1, None, Some(b"one"))]))
            .unwrap();
        let second = store
            .record_changeset(&changeset_input(
                "C:/a.package",
                vec![change_item(1, Some(b"one"), Some(b"two"))],
            ))
            .unwrap();
        // 删第一条："one" 仍被第二条的 before 引用 -> 不回收
        let result = store.delete_changeset(first.id).unwrap();
        assert_eq!(result.changesets, 1);
        assert_eq!(result.items, 1);
        assert_eq!(result.blobs_reclaimed, 0, "仍被引用的 blob 不能回收");
        {
            let connection = store.connection.lock().unwrap();
            let items: i64 = connection
                .query_row("SELECT COUNT(*) FROM change_items", [], |row| row.get(0))
                .unwrap();
            assert_eq!(items, 1, "change_items 必须级联删除");
        }
        // 再删第二条：两条 blob 都失去引用 -> 全部回收
        let result = store.delete_changeset(second.id).unwrap();
        assert_eq!(result.blobs_reclaimed, 2);
        {
            let connection = store.connection.lock().unwrap();
            let blobs: i64 = connection
                .query_row("SELECT COUNT(*) FROM resource_blobs", [], |row| row.get(0))
                .unwrap();
            assert_eq!(blobs, 0, "无引用 blob 应被回收");
        }
        // 删过的 id 再删 -> not found
        assert!(matches!(
            store.delete_changeset(first.id),
            Err(StoreError::ChangesetNotFound(_))
        ));
        drop(store);
        clean(&path);
    }

    #[test]
    fn resource_states_at_revision_handles_add_then_remove() {
        let path = temp_path("states-at-revision");
        clean(&path);
        let store = Store::open(&path).unwrap();
        // rev1 新增 1、2；rev2 改 1；rev3 删 2；rev4 再加 2
        let history: Vec<(i64, Vec<ChangeItemInput>)> = vec![
            (1, vec![change_item(1, None, Some(b"v1-1")), change_item(2, None, Some(b"v1-2"))]),
            (2, vec![change_item(1, Some(b"v1-1"), Some(b"v2-1"))]),
            (3, vec![change_item(2, Some(b"v1-2"), None)]),
            (4, vec![change_item(2, None, Some(b"v4-2"))]),
        ];
        for (_, items) in &history {
            store
                .record_changeset(&changeset_input("C:/a.package", items.clone()))
                .unwrap();
        }
        for revision in 1..=4 {
            let rows = store.resource_states_at_revision("C:/a.package", revision).unwrap();
            let actual: std::collections::BTreeMap<_, _> = rows
                .iter()
                .map(|r| {
                    (
                        (r.type_id, r.group_id, r.instance),
                        r.bytes.clone(),
                    )
                })
                .collect();
            let expected = fold_at(&history, revision);
            assert_eq!(actual, expected, "revision {revision} 的 SQL 重建必须与 fold 一致");
        }
        // rev2 期望：1 -> v2-1，2 -> v1-2
        let at_two = store.resource_states_at_revision("C:/a.package", 2).unwrap();
        assert_eq!(at_two.len(), 2);
        // rev3：2 被删
        let at_three = store.resource_states_at_revision("C:/a.package", 3).unwrap();
        assert_eq!(at_three.len(), 1);
        assert_eq!(at_three[0].instance, 1);
        drop(store);
        clean(&path);
    }

    #[test]
    fn changeset_detail_reports_before_after() {
        let path = temp_path("changeset-detail");
        clean(&path);
        let store = Store::open(&path).unwrap();
        let created = store
            .record_changeset(&changeset_input(
                "C:/a.package",
                vec![change_item(1, Some(b"old"), Some(b"newer"))],
            ))
            .unwrap();
        let (summary, items) = store.changeset_detail(created.id).unwrap();
        assert_eq!(summary.id, created.id);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].before_size, 3);
        assert_eq!(items[0].after_size, 5);
        assert!(items[0].before_digest.is_some());
        assert!(items[0].after_digest.is_some());
        assert!(matches!(
            store.changeset_detail(999_999),
            Err(StoreError::ChangesetNotFound(_))
        ));
        drop(store);
        clean(&path);
    }

    // ---------- v5: 渲染遥测 ----------

    fn telemetry(stage: &str, duration_ms: f64) -> RenderTelemetryInput {
        RenderTelemetryInput {
            session_key: "s1".into(),
            stage: stage.into(),
            trigger: "first_load".into(),
            duration_ms,
            metadata: None,
        }
    }

    #[test]
    fn render_telemetry_summary_aggregates_per_stage() {
        let path = temp_path("telemetry-summary");
        clean(&path);
        let store = Store::open(&path).unwrap();
        store
            .record_render_telemetry(&[
                telemetry("model_load", 10.0),
                telemetry("model_load", 20.0),
                telemetry("model_load", 30.0),
                telemetry("decal_render", 5.0),
            ])
            .unwrap();
        let summary = store.render_telemetry_summary(7).unwrap();
        assert_eq!(summary.total_count, 4);
        assert_eq!(summary.stages.len(), 2);
        let model = summary
            .stages
            .iter()
            .find(|s| s.stage == "model_load")
            .unwrap();
        assert_eq!(model.count, 3);
        assert!((model.avg_ms - 20.0).abs() < 1e-6);
        assert!((model.min_ms - 10.0).abs() < 1e-6);
        assert!((model.max_ms - 30.0).abs() < 1e-6);
        assert!((model.p50_ms - 20.0).abs() < 1e-6);
        assert!((model.p95_ms - 30.0).abs() < 1e-6);
        assert!((model.last_ms - 30.0).abs() < 1e-6);
        drop(store);
        clean(&path);
    }

    #[test]
    fn telemetry_prune_honors_row_cap() {
        let path = temp_path("telemetry-prune");
        clean(&path);
        let store = Store::open(&path).unwrap();
        {
            let mut connection = store.connection.lock().unwrap();
            let transaction = connection.transaction().unwrap();
            {
                let mut statement = transaction
                    .prepare(
                        "INSERT INTO render_telemetry \
                         (session_key, stage, trigger, duration_ms, metadata, created_at) \
                         VALUES ('s', 'model_load', 'first_load', 1.0, NULL, ?1)",
                    )
                    .unwrap();
                for index in 0..(RENDER_TELEMETRY_MAX_ROWS + 50) {
                    statement.execute(params![now_millis() + index]).unwrap();
                }
            }
            transaction.commit().unwrap();
        }
        let removed = store.prune_render_telemetry().unwrap();
        assert_eq!(removed, 50);
        let connection = store.connection.lock().unwrap();
        let rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM render_telemetry", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, RENDER_TELEMETRY_MAX_ROWS);
        drop(connection);
        let cleared = store.clear_render_telemetry().unwrap();
        assert_eq!(cleared.rows, RENDER_TELEMETRY_MAX_ROWS as usize);
        drop(store);
        clean(&path);
    }

    /// 性能报告（项目约定：每阶段测试附耗时）。运行：`cargo test -p sc-store -- --nocapture perf_`
    #[test]
    fn perf_migrate_and_record_changeset() {
        use std::time::Instant;

        let path = temp_path("perf");
        clean(&path);
        let start = Instant::now();
        let store = Store::open(&path).unwrap();
        let migrate_ms = start.elapsed().as_secs_f64() * 1000.0;

        // 100 条 8 KB 负载
        let payload = vec![0xABu8; 8 * 1024];
        let items: Vec<ChangeItemInput> = (0..100)
            .map(|index| change_item(index as u32, None, Some(&payload)))
            .collect();
        let start = Instant::now();
        for _ in 0..5 {
            store
                .record_changeset(&changeset_input("C:/perf.package", items.clone()))
                .unwrap();
        }
        let record_ms = start.elapsed().as_secs_f64() * 1000.0 / 5.0;

        // 1 万行遥测的汇总
        let entries: Vec<RenderTelemetryInput> = (0..10_000)
            .map(|index| telemetry("model_load", index as f64))
            .collect();
        store.record_render_telemetry(&entries).unwrap();
        let start = Instant::now();
        let summary = store.render_telemetry_summary(30).unwrap();
        let summary_ms = start.elapsed().as_secs_f64() * 1000.0;

        println!(
            "[perf sc-store] migrate(首次)={migrate_ms:.2}ms  record_changeset(100×8KB)={record_ms:.2}ms  \
             summary({} 行)={summary_ms:.2}ms",
            summary.total_count
        );

        assert!(migrate_ms < 500.0, "migrate 过慢：{migrate_ms}ms");
        assert!(record_ms < 300.0, "record_changeset 过慢：{record_ms}ms");
        assert!(summary_ms < 200.0, "summary 过慢：{summary_ms}ms");
        drop(store);
        clean(&path);
    }
}
