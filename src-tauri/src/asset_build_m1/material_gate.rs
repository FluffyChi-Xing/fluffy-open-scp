//! 材质指纹闸门（G1，用户指令 2026-11-05："像 128 列这样关键信息必须打印
//! 日志；材质合成这种基本能力不允许发生回归"）。
//!
//! s10 事故形态：模型输入从 1 材质组×128 参数列换成 74 组×1 列，构建
//! "成功"而立面贴图全丢。本闸门在 insert-model 边界对实际构建产物指纹
//! 做逐字段校验：
//! - 指纹与上次成功构建一致 → 放行（纯场景变更的安全通道）；
//! - 指纹漂移且 schema 未声明 `semantics.materialChange: true` → 构建失败，
//!   错误逐字段列出 old→new（`[insert-model] material fingerprint ...`）。
//!
//! 指纹随阶段事件与台账落盘（`materialFingerprint`），关键数字肉眼可查。
//! 基线 = `.build/material-baseline.json`，按模型路径键控，每次放行后刷新。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

const BASELINE_FILE: &str = ".build/material-baseline.json";

/// insert-model 边界可观测的材质身份。字段对齐 s10 事故报告的 Required
/// regression gates（源模型 hash / 材质组数 / 参数列 / 网格数 / 六槽
/// 实例与载荷 / 材质资源数）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MaterialFingerprint {
    pub model_sha256: String,
    pub model_shape: Option<String>,
    pub material_groups: usize,
    pub parameter_columns: Vec<u32>,
    pub mesh_count: usize,
    pub texture_slot_instances: [u32; 6],
    /// 六槽源文件字节的串联摘要（实例号可能因池内分配漂移，载荷不会）。
    pub slot_payload_sha256: String,
    pub material_resources: usize,
}

#[derive(Debug)]
pub(crate) enum GateOutcome {
    /// 项目内无该模型的基线：记录并放行（首建）。
    Recorded,
    /// 与上次成功构建逐字段一致。
    Unchanged,
    /// 指纹漂移且 schema 显式声明了 materialChange；基线已刷新。
    DeclaredChange(Vec<String>),
}

impl GateOutcome {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            GateOutcome::Recorded => "recorded",
            GateOutcome::Unchanged => "unchanged",
            GateOutcome::DeclaredChange(_) => "declared-change",
        }
    }
}

type Baseline = BTreeMap<String, MaterialFingerprint>;

fn load_baseline(root: &Path) -> Result<Baseline, String> {
    let path = root.join(BASELINE_FILE);
    if !path.is_file() {
        return Ok(Baseline::new());
    }
    let bytes = std::fs::read(&path).map_err(|e| format!("material baseline read: {e}"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("material baseline decode: {e}"))
}

fn store_baseline(root: &Path, baseline: &Baseline) -> Result<(), String> {
    let path = root.join(BASELINE_FILE);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("material baseline dir: {e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(baseline)
        .map_err(|e| format!("material baseline encode: {e}"))?;
    match crate::atomic_fs::write_atomic(&path, &bytes, true) {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(5) => {
            // Windows managed project folders can reject the atomic rename
            // while allowing a direct create/replace of this derived ledger.
            std::fs::write(&path, &bytes).map_err(|e| format!("material baseline write: {e}"))
        }
        Err(error) => Err(format!("material baseline write: {error}")),
    }
}

/// 字段级差异（顺序即输出顺序）。128→1 这类塌缩在第一行即可读出。
pub(crate) fn diff_fingerprint(
    old: &MaterialFingerprint,
    new: &MaterialFingerprint,
) -> Vec<String> {
    let mut lines = Vec::new();
    if old.model_sha256 != new.model_sha256 {
        lines.push(format!(
            "modelSha256 {} -> {}",
            old.model_sha256, new.model_sha256
        ));
    }
    if old.model_shape != new.model_shape {
        lines.push(format!(
            "modelShape {:?} -> {:?}",
            old.model_shape, new.model_shape
        ));
    }
    if old.material_groups != new.material_groups {
        lines.push(format!(
            "materialGroups {} -> {}",
            old.material_groups, new.material_groups
        ));
    }
    if old.parameter_columns != new.parameter_columns {
        lines.push(format!(
            "parameterColumns {:?} -> {:?}",
            old.parameter_columns, new.parameter_columns
        ));
    }
    if old.mesh_count != new.mesh_count {
        lines.push(format!(
            "meshCount {} -> {}",
            old.mesh_count, new.mesh_count
        ));
    }
    if old.texture_slot_instances != new.texture_slot_instances {
        lines.push(format!(
            "textureSlotInstances {:08X?} -> {:08X?}",
            old.texture_slot_instances, new.texture_slot_instances
        ));
    }
    if old.slot_payload_sha256 != new.slot_payload_sha256 {
        lines.push(format!(
            "slotPayloadSha256 {} -> {}",
            old.slot_payload_sha256, new.slot_payload_sha256
        ));
    }
    if old.material_resources != new.material_resources {
        lines.push(format!(
            "materialResources {} -> {}",
            old.material_resources, new.material_resources
        ));
    }
    lines
}

/// 闸门主体。失败错误固定携带责任阶段名，与设计文档的不变量错误格式一致。
pub(crate) fn enforce(
    root: &Path,
    model_key: &str,
    fingerprint: &MaterialFingerprint,
    declared_change: bool,
) -> Result<GateOutcome, String> {
    let mut baseline = load_baseline(root)?;
    let outcome = match baseline.get(model_key) {
        None => {
            baseline.insert(model_key.to_string(), fingerprint.clone());
            store_baseline(root, &baseline)?;
            GateOutcome::Recorded
        }
        Some(old) if old == fingerprint => GateOutcome::Unchanged,
        Some(old) => {
            let diff = diff_fingerprint(old, fingerprint);
            if !declared_change {
                return Err(format!(
                    "[insert-model] material fingerprint changed without semantics.materialChange=true (s10 材质回归闸门):\n  {}",
                    diff.join("\n  ")
                ));
            }
            baseline.insert(model_key.to_string(), fingerprint.clone());
            store_baseline(root, &baseline)?;
            GateOutcome::DeclaredChange(diff)
        }
    };
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(columns: Vec<u32>) -> MaterialFingerprint {
        MaterialFingerprint {
            model_sha256: "aa".into(),
            model_shape: Some("building40".into()),
            material_groups: 1,
            parameter_columns: columns,
            mesh_count: 74,
            texture_slot_instances: [1, 2, 3, 0, 0, 0],
            slot_payload_sha256: "bb".into(),
            material_resources: 3,
        }
    }

    fn temp_root(label: &str) -> std::path::PathBuf {
        let root = crate::test_scratch::scratch_base().join(format!(
            "openscp-material-gate-{label}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn first_build_records_then_unchanged_passes() {
        let root = temp_root("record");
        let fp = fingerprint(vec![128]);
        assert!(matches!(
            enforce(&root, "model.lotm", &fp, false).unwrap(),
            GateOutcome::Recorded
        ));
        assert!(matches!(
            enforce(&root, "model.lotm", &fp, false).unwrap(),
            GateOutcome::Unchanged
        ));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn silent_fingerprint_drift_is_rejected_with_field_diff() {
        let root = temp_root("drift");
        let fp = fingerprint(vec![128]);
        enforce(&root, "model.lotm", &fp, false).unwrap();
        // s10 形态：128 列塌缩成 74 组×1 列。
        let mut drifted = fingerprint(vec![1; 74]);
        drifted.material_groups = 74;
        let error = enforce(&root, "model.lotm", &drifted, false).unwrap_err();
        assert!(error.contains("[insert-model]"), "{error}");
        assert!(error.contains("parameterColumns"), "{error}");
        assert!(error.contains("128"), "{error}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn declared_change_passes_and_refreshes_baseline() {
        let root = temp_root("declared");
        let fp = fingerprint(vec![128]);
        enforce(&root, "model.lotm", &fp, false).unwrap();
        let mut drifted = fingerprint(vec![64]);
        drifted.model_sha256 = "cc".into();
        let outcome = enforce(&root, "model.lotm", &drifted, true).unwrap();
        assert!(matches!(outcome, GateOutcome::DeclaredChange(_)));
        // 基线已刷新：再次构建（不声明）按新指纹放行。
        assert!(matches!(
            enforce(&root, "model.lotm", &drifted, false).unwrap(),
            GateOutcome::Unchanged
        ));
        let _ = std::fs::remove_dir_all(&root);
    }
}
