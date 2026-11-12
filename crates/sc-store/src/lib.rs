use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CURRENT_SCHEMA_VERSION: i32 = 7;
pub const DEFAULT_LIST_LIMIT: usize = 100;
pub const MAX_LIST_LIMIT: usize = 1_000;

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
    #[error("mod project {0} was not found")]
    ModProjectNotFound(i64),
    #[error("mod project group {0} was not found")]
    ModGroupNotFound(i64),
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
        let mod_root = mod_root
            .map_or(current_root.as_deref(), Some)
            .map(str::to_owned);
        let onboarding = if onboarding_completed {
            true
        } else {
            current_onboarding
        };
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
            let mut statement = connection
                .prepare("SELECT group_id, COUNT(*) FROM mod_projects GROUP BY group_id")?;
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
            let mut statement = connection.prepare(
                "SELECT status, COUNT(*) FROM mod_projects GROUP BY status ORDER BY status",
            )?;
            let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        if status_counts.is_empty() {
            status_counts.push(("active".into(), 0));
        }
        let timestamps = {
            let mut statement =
                connection.prepare("SELECT created_at, updated_at FROM mod_projects")?;
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
        // 保留历史迁移格式。版本控制台移除后，应用不再读写这些版本记录。
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
    if version <= 6 {
        connection.execute_batch(
            "BEGIN;
             CREATE TABLE effect_annotations (
                 id INTEGER PRIMARY KEY,
                 type_id INTEGER NOT NULL,
                 group_id INTEGER NOT NULL,
                 instance INTEGER NOT NULL,
                 tag TEXT NOT NULL,
                 note TEXT NOT NULL DEFAULT '',
                 source_type_id INTEGER,
                 source_group_id INTEGER,
                 source_instance INTEGER,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE INDEX effect_annotations_tgi_idx
                 ON effect_annotations(type_id, group_id, instance);
             CREATE INDEX effect_annotations_tag_idx
                 ON effect_annotations(tag);
             CREATE INDEX effect_annotations_source_idx
                 ON effect_annotations(source_type_id, source_group_id, source_instance);
             PRAGMA user_version = 7;
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

/// Effect 语义批注：按 effect 资源 TGI 全局键控（跨资产复用，组件库按
/// tag 分类），`source_*` 记录创建时的来源 lot 资源（批注 sheet 区域用，
/// 在 draft 工作台创建时为空）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EffectAnnotation {
    pub id: i64,
    pub type_id: u32,
    pub group_id: u32,
    pub instance: u32,
    pub tag: String,
    pub note: String,
    pub source_type_id: Option<u32>,
    pub source_group_id: Option<u32>,
    pub source_instance: Option<u32>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EffectAnnotationInput {
    pub type_id: u32,
    pub group_id: u32,
    pub instance: u32,
    pub tag: String,
    pub note: String,
    pub source_type_id: Option<u32>,
    pub source_group_id: Option<u32>,
    pub source_instance: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EffectTagStat {
    pub tag: String,
    pub annotation_count: i64,
    pub effect_count: i64,
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
    // ---------- effect 语义批注 ----------

    fn row_to_effect_annotation(row: &rusqlite::Row<'_>) -> rusqlite::Result<EffectAnnotation> {
        Ok(EffectAnnotation {
            id: row.get(0)?,
            type_id: row.get::<_, i64>(1)? as u32,
            group_id: row.get::<_, i64>(2)? as u32,
            instance: row.get::<_, i64>(3)? as u32,
            tag: row.get(4)?,
            note: row.get(5)?,
            source_type_id: row.get::<_, Option<i64>>(6)?.map(|value| value as u32),
            source_group_id: row.get::<_, Option<i64>>(7)?.map(|value| value as u32),
            source_instance: row.get::<_, Option<i64>>(8)?.map(|value| value as u32),
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
        })
    }

    pub fn create_effect_annotation(
        &self,
        input: &EffectAnnotationInput,
    ) -> Result<EffectAnnotation> {
        let now = now_millis();
        let connection = self.connection.lock().expect("store mutex poisoned");
        connection.execute(
            "INSERT INTO effect_annotations
                 (type_id, group_id, instance, tag, note, source_type_id,
                  source_group_id, source_instance, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                input.type_id as i64,
                input.group_id as i64,
                input.instance as i64,
                input.tag.trim(),
                input.note,
                input.source_type_id.map(|value| value as i64),
                input.source_group_id.map(|value| value as i64),
                input.source_instance.map(|value| value as i64),
                now
            ],
        )?;
        let id = connection.last_insert_rowid();
        Ok(EffectAnnotation {
            id,
            type_id: input.type_id,
            group_id: input.group_id,
            instance: input.instance,
            tag: input.tag.trim().to_string(),
            note: input.note.clone(),
            source_type_id: input.source_type_id,
            source_group_id: input.source_group_id,
            source_instance: input.source_instance,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn delete_effect_annotation(&self, id: i64) -> Result<()> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let changed =
            connection.execute("DELETE FROM effect_annotations WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(StoreError::OperationNotFound(id));
        }
        Ok(())
    }

    pub fn list_effect_annotations(&self, limit: usize) -> Result<Vec<EffectAnnotation>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, type_id, group_id, instance, tag, note, source_type_id, \
                    source_group_id, source_instance, created_at, updated_at
             FROM effect_annotations ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let rows = statement
            .query_map(
                params![bounded_limit(limit) as i64],
                Self::row_to_effect_annotation,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn list_effect_annotations_for_effect(
        &self,
        type_id: u32,
        group_id: u32,
        instance: u32,
    ) -> Result<Vec<EffectAnnotation>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, type_id, group_id, instance, tag, note, source_type_id, \
                    source_group_id, source_instance, created_at, updated_at
             FROM effect_annotations
             WHERE type_id = ?1 AND group_id = ?2 AND instance = ?3
             ORDER BY updated_at DESC",
        )?;
        let rows = statement
            .query_map(
                params![type_id as i64, group_id as i64, instance as i64],
                Self::row_to_effect_annotation,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn list_effect_annotations_for_source(
        &self,
        type_id: u32,
        group_id: u32,
        instance: u32,
    ) -> Result<Vec<EffectAnnotation>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id, type_id, group_id, instance, tag, note, source_type_id, \
                    source_group_id, source_instance, created_at, updated_at
             FROM effect_annotations
             WHERE source_type_id = ?1 AND source_group_id = ?2 AND source_instance = ?3
             ORDER BY updated_at DESC",
        )?;
        let rows = statement
            .query_map(
                params![type_id as i64, group_id as i64, instance as i64],
                Self::row_to_effect_annotation,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn effect_annotation_tag_stats(&self) -> Result<Vec<EffectTagStat>> {
        let connection = self.connection.lock().expect("store mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT tag,
                    COUNT(*) AS annotation_count,
                    COUNT(DISTINCT type_id || ':' || group_id || ':' || instance) AS effect_count
             FROM effect_annotations GROUP BY tag ORDER BY annotation_count DESC",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(EffectTagStat {
                    tag: row.get(0)?,
                    annotation_count: row.get(1)?,
                    effect_count: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

impl Store {
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
                let metadata = entry
                    .metadata
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?;
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

/// 最近秩（nearest-rank）分位数，`sorted` 必须已升序。
fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (q * (sorted.len() - 1) as f64).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
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
            // 人为降级到 v4：删掉 v5/v6/v7 的表并回写 user_version
            {
                let connection = store.connection.lock().unwrap();
                connection
                    .execute_batch(
                        "BEGIN;
                         DROP TABLE effect_annotations;
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
        drop(store);
        clean(&path);
    }

    #[test]
    fn effect_annotations_crud_tag_stats_and_source_query() {
        let path = temp_path("effect-annotations");
        clean(&path);
        let store = Store::open(&path).unwrap();
        let first = store
            .create_effect_annotation(&EffectAnnotationInput {
                type_id: 0,
                group_id: 0,
                instance: 0x622b_9cd7,
                tag: "烟雾".into(),
                note: "烟囱烟雾".into(),
                source_type_id: Some(0x00b1_b104),
                source_group_id: Some(0x43e1_c090),
                source_instance: Some(0x9a3d_1c76),
            })
            .unwrap();
        store
            .create_effect_annotation(&EffectAnnotationInput {
                type_id: 0,
                group_id: 0,
                instance: 0xfa37_82d1,
                tag: "烟雾".into(),
                note: String::new(),
                source_type_id: None,
                source_group_id: None,
                source_instance: None,
            })
            .unwrap();
        store
            .create_effect_annotation(&EffectAnnotationInput {
                type_id: 0,
                group_id: 0,
                instance: 0x1122_3344,
                tag: "火焰".into(),
                note: String::new(),
                source_type_id: None,
                source_group_id: None,
                source_instance: None,
            })
            .unwrap();

        // 同一 effect 再标一个 tag：多对多，全局键控
        store
            .create_effect_annotation(&EffectAnnotationInput {
                type_id: 0,
                group_id: 0,
                instance: 0x622b_9cd7,
                tag: "工业".into(),
                note: String::new(),
                source_type_id: None,
                source_group_id: None,
                source_instance: None,
            })
            .unwrap();

        let all = store.list_effect_annotations(100).unwrap();
        assert_eq!(all.len(), 4);

        let per_effect = store
            .list_effect_annotations_for_effect(0, 0, 0x622b_9cd7)
            .unwrap();
        assert_eq!(per_effect.len(), 2);

        let per_source = store
            .list_effect_annotations_for_source(0x00b1_b104, 0x43e1_c090, 0x9a3d_1c76)
            .unwrap();
        assert_eq!(per_source.len(), 1);
        assert_eq!(per_source[0].tag, "烟雾");

        let mut stats = store.effect_annotation_tag_stats().unwrap();
        stats.sort_by(|a, b| a.tag.cmp(&b.tag));
        assert_eq!(stats.len(), 3);
        assert_eq!(stats[2].tag, "烟雾"); // annotation_count=2, 排序后最后
        assert_eq!(stats[2].annotation_count, 2);
        assert_eq!(stats[2].effect_count, 2);

        store.delete_effect_annotation(first.id).unwrap();
        assert!(
            store
                .list_effect_annotations_for_effect(0, 0, 0x622b_9cd7)
                .unwrap()
                .iter()
                .all(|item| item.tag != "烟雾")
        );
        drop(store);
        clean(&path);
    }

    #[test]
    fn v5_database_migrates_to_v6_keeping_changesets() {
        let path = temp_path("v5-to-v6");
        clean(&path);
        {
            let store = Store::open(&path).unwrap();
            // 人为降级到 v5：删掉 v6/v7 的表并回写 user_version
            {
                let connection = store.connection.lock().unwrap();
                connection
                    .execute_batch(
                        "BEGIN;
                         DROP TABLE effect_annotations;
                         DROP TABLE studio_config;
                         DROP TABLE mod_projects;
                         DROP TABLE mod_project_groups;
                         INSERT INTO changesets
                             (target_path, writer, revision, resource_count, created_at)
                             VALUES ('C:/a.package', 'test', 1, 0, 1);
                         PRAGMA user_version = 5;
                         COMMIT;",
                    )
                    .unwrap();
            }
        }
        let store = Store::open(&path).unwrap();
        let target: String = store
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT target_path FROM changesets", [], |row| row.get(0))
            .unwrap();
        assert_eq!(target, "C:/a.package");
        // v6 表当场可用
        let config = store.studio_config().unwrap();
        assert!(!config.onboarding_completed);
        assert_eq!(config.mod_root, None);
        drop(store);
        clean(&path);
    }

    #[test]
    fn v6_database_migrates_to_v7_keeping_annotations() {
        let path = temp_path("v6-to-v7");
        clean(&path);
        {
            let store = Store::open(&path).unwrap();
            store
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
            // 人为降级到 v6：删掉 v7 的表并回写 user_version
            {
                let connection = store.connection.lock().unwrap();
                connection
                    .execute_batch(
                        "BEGIN;
                         DROP TABLE effect_annotations;
                         PRAGMA user_version = 6;
                         COMMIT;",
                    )
                    .unwrap();
            }
        }
        let store = Store::open(&path).unwrap();
        {
            let connection = store.connection.lock().unwrap();
            let version: i32 = connection
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(version, CURRENT_SCHEMA_VERSION);
        }
        assert_eq!(store.list_annotations(100).unwrap().len(), 1);
        // v7 表当场可用
        assert_eq!(
            store.list_effect_annotations(100).unwrap(),
            Vec::<EffectAnnotation>::new()
        );
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
        let moved = store
            .mod_project_set_group(created.id, Some(group2.id))
            .unwrap();
        assert_eq!(moved.group_id, Some(group2.id));
        store
            .mod_project_set_status(created.id, "released")
            .unwrap();
        let described = store.mod_project_set_description(created.id, None).unwrap();
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
            .query_row("SELECT COUNT(*) FROM render_telemetry", [], |row| {
                row.get(0)
            })
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
    fn perf_migrate_and_summarize_telemetry() {
        use std::time::Instant;

        let path = temp_path("perf");
        clean(&path);
        let start = Instant::now();
        let store = Store::open(&path).unwrap();
        let migrate_ms = start.elapsed().as_secs_f64() * 1000.0;

        // 1 万行遥测的汇总
        let entries: Vec<RenderTelemetryInput> = (0..10_000)
            .map(|index| telemetry("model_load", index as f64))
            .collect();
        store.record_render_telemetry(&entries).unwrap();
        let start = Instant::now();
        let summary = store.render_telemetry_summary(30).unwrap();
        let summary_ms = start.elapsed().as_secs_f64() * 1000.0;

        println!(
            "[perf sc-store] migrate(首次)={migrate_ms:.2}ms  \
             summary({} 行)={summary_ms:.2}ms",
            summary.total_count
        );

        assert!(migrate_ms < 500.0, "migrate 过慢：{migrate_ms}ms");
        assert!(summary_ms < 200.0, "summary 过慢：{summary_ms}ms");
        drop(store);
        clean(&path);
    }
}
