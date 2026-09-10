//! 资源批注（模组资产笔记）：源文件右键创建/编辑批注，md 内容存 SQLite
//! （sc-store `resource_annotations` 表，schema v4）。工作区笔记页面消费
//! 全量列表与按 topic 的统计。

use serde::Deserialize;
use tauri::State;

use crate::activity::{AppState, CommandError};
use sc_store::{ResourceAnnotation, ResourceAnnotationInput};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationTgiRequest {
    pub type_id: u32,
    pub group_id: u32,
    pub instance: u32,
}

#[tauri::command]
pub async fn annotation_create(
    state: State<'_, AppState>,
    input: ResourceAnnotationInput,
) -> Result<ResourceAnnotation, CommandError> {
    if input.title.trim().is_empty() {
        return Err(CommandError::internal("annotation title is required"));
    }
    if input.topic.trim().is_empty() {
        return Err(CommandError::internal("annotation topic is required"));
    }
    state
        .store
        .create_annotation(&input)
        .map_err(CommandError::from)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationUpdateRequest {
    pub id: i64,
    pub topic: String,
    pub title: String,
    pub content: String,
}

#[tauri::command]
pub async fn annotation_update(
    state: State<'_, AppState>,
    request: AnnotationUpdateRequest,
) -> Result<ResourceAnnotation, CommandError> {
    state
        .store
        .update_annotation(request.id, &request.topic, &request.title, &request.content)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn annotation_delete(state: State<'_, AppState>, id: i64) -> Result<(), CommandError> {
    state
        .store
        .delete_annotation(id)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn annotations_for_tgi(
    state: State<'_, AppState>,
    request: AnnotationTgiRequest,
) -> Result<Vec<ResourceAnnotation>, CommandError> {
    state
        .store
        .list_annotations_for_tgi(request.type_id, request.group_id, request.instance)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn annotations_list(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<ResourceAnnotation>, CommandError> {
    state
        .store
        .list_annotations(limit.unwrap_or(sc_store::MAX_LIST_LIMIT))
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn annotation_topic_stats(
    state: State<'_, AppState>,
) -> Result<Vec<sc_store::AnnotationTopicStat>, CommandError> {
    state
        .store
        .annotation_topic_stats()
        .map_err(CommandError::from)
}
