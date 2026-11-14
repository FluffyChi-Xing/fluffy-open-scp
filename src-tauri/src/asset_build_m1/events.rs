//! 构建阶段事件(P1 责任链埋点契约,2026-11-04)。
//!
//! 每个 SectionProcessor 在边界处产出 `StageEvent`,随 `M1BuildResult.events`
//! 返回;P2 的事件队列/OpenSCP Actions 页直接消费本类型(持久化 + tauri
//! emit 走 activity.rs 的 ACTIVITY_EVENT 同款模式)。事件不参与 build_hash
//! 与产物字节——纯观测面。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageEvent {
    /// 阶段名(= 责任链节点名,Actions 页的"步骤"行)。
    pub stage: String,
    pub kind: StageEventKind,
    /// Wall-clock timestamp for the event (Unix epoch milliseconds).
    #[serde(default)]
    pub timestamp_ms: u64,
    /// Elapsed time from the matching started event. Only present on
    /// finished/failed events; this keeps the event stream append-only.
    pub duration_ms: Option<u64>,
    /// 阶段专属详情(计数/路径/错误文本),JSON 透传。
    pub detail: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum StageEventKind {
    Started,
    Finished,
    Failed,
}

pub fn started(stage: impl Into<String>, detail: serde_json::Value) -> StageEvent {
    StageEvent {
        stage: stage.into(),
        kind: StageEventKind::Started,
        timestamp_ms: now_ms(),
        duration_ms: None,
        detail,
    }
}

pub fn finished(
    stage: impl Into<String>,
    detail: serde_json::Value,
    duration_ms: u64,
) -> StageEvent {
    StageEvent {
        stage: stage.into(),
        kind: StageEventKind::Finished,
        timestamp_ms: now_ms(),
        duration_ms: Some(duration_ms),
        detail,
    }
}

pub fn failed(stage: impl Into<String>, error: &str, duration_ms: u64) -> StageEvent {
    StageEvent {
        stage: stage.into(),
        kind: StageEventKind::Failed,
        timestamp_ms: now_ms(),
        duration_ms: Some(duration_ms),
        detail: serde_json::Value::String(error.to_string()),
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_protocol_keeps_step_timing_fields() {
        let started = started("insert-model", serde_json::json!({"meshCount": 74}));
        let finished = finished("insert-model", serde_json::json!({"materials": 1}), 12);
        assert_eq!(started.stage, finished.stage);
        assert!(started.timestamp_ms > 0);
        assert_eq!(started.duration_ms, None);
        assert_eq!(finished.duration_ms, Some(12));
    }
}
