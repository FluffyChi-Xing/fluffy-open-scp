use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CURRENT_SCHEMA_VERSION: i32 = 3;
pub const DEFAULT_LIST_LIMIT: usize = 100;
pub const MAX_LIST_LIMIT: usize = 1_000;

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
}

#[derive(Debug)]
pub struct Store {
    connection: Mutex<Connection>,
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
        Ok(Self {
            connection: Mutex::new(connection),
        })
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
}

fn migrate(connection: &Connection) -> Result<()> {
    connection.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    let version: i32 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
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
        return Ok(());
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
    }
    Ok(())
}

fn bounded_limit(limit: usize) -> usize {
    limit.clamp(1, MAX_LIST_LIMIT)
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
}
