use std::sync::Arc;

use sc_store::{ActivityClearResult, Event, Operation, Package, Store, StoreError};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::package_service::PackageManager;

pub const ACTIVITY_EVENT: &str = "activity:event";

pub struct AppState {
    pub(crate) store: Arc<Store>,
    pub(crate) packages: Arc<PackageManager>,
    pub(crate) app: AppHandle,
}

impl AppState {
    pub fn new(app: AppHandle, store: Store) -> Self {
        Self {
            store: Arc::new(store),
            packages: Arc::new(PackageManager::new()),
            app,
        }
    }

    #[allow(dead_code)]
    pub fn append_event(&self, input: &sc_store::EventInput) -> Result<Event, StoreError> {
        let event = self.store.append_event(input)?;
        let _ = self.app.emit(ACTIVITY_EVENT, &event);
        Ok(event)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub(crate) fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new("internal_error", message)
    }
}

impl From<StoreError> for CommandError {
    fn from(error: StoreError) -> Self {
        Self {
            code: "store_error".into(),
            message: error.to_string(),
        }
    }
}

#[tauri::command]
pub async fn activity_list_operations(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<Operation>, CommandError> {
    let store = Arc::clone(&state.store);
    let limit = limit.unwrap_or(sc_store::DEFAULT_LIST_LIMIT);
    tauri::async_runtime::spawn_blocking(move || store.list_operations(limit))
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn activity_list_events(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<Event>, CommandError> {
    let store = Arc::clone(&state.store);
    let limit = limit.unwrap_or(sc_store::DEFAULT_LIST_LIMIT);
    tauri::async_runtime::spawn_blocking(move || store.list_events(limit))
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn activity_list_packages(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<Package>, CommandError> {
    let store = Arc::clone(&state.store);
    let limit = limit.unwrap_or(sc_store::DEFAULT_LIST_LIMIT);
    tauri::async_runtime::spawn_blocking(move || store.list_packages(limit))
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn activity_clear(
    state: State<'_, AppState>,
) -> Result<ActivityClearResult, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || store.clear_activity())
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
        .map_err(CommandError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_channel_name_is_stable() {
        assert_eq!(ACTIVITY_EVENT, "activity:event");
    }

    #[test]
    fn command_error_is_serializable() {
        let error = CommandError::internal("database unavailable");
        assert_eq!(
            serde_json::to_value(error).unwrap()["code"],
            "internal_error"
        );
    }
}
