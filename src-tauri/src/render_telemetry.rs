//! 渲染可观测性：把前端各渲染阶段（模型加载 / 贴图合成 / lot / decal / 场景重建）
//! 的耗时与元数据落库，供属性编辑器的「渲染遥测」页签与仪表盘统计卡读取。
//!
//! 与渲染引擎解耦：后端只负责存储与聚合，不参与、也不影响渲染本身。

use std::sync::Arc;

use sc_store::{RenderTelemetryInput, RenderTelemetrySummary, TelemetryClearResult};
use serde::Deserialize;
use tauri::State;

use crate::activity::{AppState, CommandError};

/// 默认统计窗口（天）。
const DEFAULT_WINDOW_DAYS: i64 = 7;
/// 单次批量上报的记录数上限（前端 250 ms 去抖，正常远小于此）。
const MAX_BATCH_ENTRIES: usize = 512;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordRenderTelemetryRequest {
    pub entries: Vec<RenderTelemetryInput>,
}

#[tauri::command]
pub async fn render_telemetry_record(
    state: State<'_, AppState>,
    request: RecordRenderTelemetryRequest,
) -> Result<usize, CommandError> {
    if request.entries.is_empty() {
        return Ok(0);
    }
    if request.entries.len() > MAX_BATCH_ENTRIES {
        return Err(CommandError::new(
            "invalid_argument",
            format!("too many telemetry entries (limit {MAX_BATCH_ENTRIES})"),
        ));
    }
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || store.record_render_telemetry(&request.entries))
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn render_telemetry_summary(
    state: State<'_, AppState>,
    days: Option<i64>,
) -> Result<RenderTelemetrySummary, CommandError> {
    let store = Arc::clone(&state.store);
    let days = days.unwrap_or(DEFAULT_WINDOW_DAYS);
    tauri::async_runtime::spawn_blocking(move || store.render_telemetry_summary(days))
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn render_telemetry_clear(
    state: State<'_, AppState>,
) -> Result<TelemetryClearResult, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || store.clear_render_telemetry())
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
        .map_err(CommandError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_request_deserializes_from_camel_case() {
        let request: RecordRenderTelemetryRequest = serde_json::from_str(
            r#"{"entries":[{"sessionKey":"s","stage":"model_load","trigger":"first_load",
                 "durationMs":12.5,"metadata":{"meshes":3}}]}"#,
        )
        .unwrap();
        assert_eq!(request.entries.len(), 1);
        assert_eq!(request.entries[0].stage, "model_load");
        assert!((request.entries[0].duration_ms - 12.5).abs() < 1e-6);
        assert_eq!(
            request.entries[0]
                .metadata
                .as_ref()
                .and_then(|value| value.get("meshes"))
                .and_then(|value| value.as_i64()),
            Some(3)
        );
    }
}
