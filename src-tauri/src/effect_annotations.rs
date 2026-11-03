//! Effect 语义批注（效果 tag 分类）：property viewer 选中 effect 后创建
//! 语义 tag + 批注，存 SQLite（sc-store `effect_annotations` 表，schema
//! v7）。批注 sheet 区域、笔记工作区 Effect tab 与资产编辑器组件库效果
//! 页签消费。

use serde::Deserialize;
use tauri::State;

use crate::activity::{AppState, CommandError};
use sc_store::{EffectAnnotation, EffectAnnotationInput};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectAnnotationTgiRequest {
    pub type_id: u32,
    pub group_id: u32,
    pub instance: u32,
}

#[tauri::command]
pub async fn effect_annotation_create(
    state: State<'_, AppState>,
    input: EffectAnnotationInput,
) -> Result<EffectAnnotation, CommandError> {
    if input.tag.trim().is_empty() {
        return Err(CommandError::internal("effect annotation tag is required"));
    }
    state
        .store
        .create_effect_annotation(&input)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn effect_annotation_delete(
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), CommandError> {
    state
        .store
        .delete_effect_annotation(id)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn effect_annotations_list(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<EffectAnnotation>, CommandError> {
    state
        .store
        .list_effect_annotations(limit.unwrap_or(sc_store::MAX_LIST_LIMIT))
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn effect_annotations_for_effect(
    state: State<'_, AppState>,
    request: EffectAnnotationTgiRequest,
) -> Result<Vec<EffectAnnotation>, CommandError> {
    state
        .store
        .list_effect_annotations_for_effect(request.type_id, request.group_id, request.instance)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn effect_annotations_for_source(
    state: State<'_, AppState>,
    request: EffectAnnotationTgiRequest,
) -> Result<Vec<EffectAnnotation>, CommandError> {
    state
        .store
        .list_effect_annotations_for_source(request.type_id, request.group_id, request.instance)
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn effect_annotation_tag_stats(
    state: State<'_, AppState>,
) -> Result<Vec<sc_store::EffectTagStat>, CommandError> {
    state
        .store
        .effect_annotation_tag_stats()
        .map_err(CommandError::from)
}
