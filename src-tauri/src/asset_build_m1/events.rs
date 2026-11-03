//! 构建阶段事件(P1 责任链埋点契约,2026-11-04)。
//!
//! 每个 SectionProcessor 在边界处产出 `StageEvent`,随 `M1BuildResult.events`
//! 返回;P2 的事件队列/OpenSCP Actions 页直接消费本类型(持久化 + tauri
//! emit 走 activity.rs 的 ACTIVITY_EVENT 同款模式)。事件不参与 build_hash
//! 与产物字节——纯观测面。

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageEvent {
    /// 阶段名(= 责任链节点名,Actions 页的"步骤"行)。
    pub stage: &'static str,
    pub kind: StageEventKind,
    /// 阶段专属详情(计数/路径/错误文本),JSON 透传。
    pub detail: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum StageEventKind {
    Started,
    Finished,
    Failed,
}

pub fn started(stage: &'static str, detail: serde_json::Value) -> StageEvent {
    StageEvent { stage, kind: StageEventKind::Started, detail }
}

pub fn finished(stage: &'static str, detail: serde_json::Value) -> StageEvent {
    StageEvent { stage, kind: StageEventKind::Finished, detail }
}

pub fn failed(stage: &'static str, error: &str) -> StageEvent {
    StageEvent {
        stage,
        kind: StageEventKind::Failed,
        detail: serde_json::Value::String(error.to_string()),
    }
}
