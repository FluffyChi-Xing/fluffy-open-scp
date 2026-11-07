//! delta-audit 闸门（2026-11-07，r10 事故后定案）。
//!
//! 语义：**最终 Lot 属性 vs 参照 Lot 属性逐列语义对拍**。只有"显式声明"
//! 的列允许变化：
//!
//! - `scene.writes` 声明的列族（`declared_column_hashes`，与写回共用同一张表）；
//! - 场景文档显式给出的 LotSize / OverlayOffset / Placement；
//! - LOD 模型引用列（每次构建按确定性身份重定向，模型存在性由
//!   insert-model 不变量与材质闸门负责）；
//! - `semantics.properties` 显式 patch 的列；
//! - `semantics.behavior` 声明的路网/看板列（行为原语自带不变量）。
//!
//! 其它任何 changed / added / removed = 构建失败。失败发生在
//! materialize-and-link **之前**，因此上一次成功产物不会被覆盖——
//! 这就是"path 没了不会连带模型没了"的机制保证：写回按列族隔离，
//! 审计按列族点名，发布保持原子。
//!
//! r10 事故形态（工作台文档整体重写 lot，未建模的锚点/灯光列被清空）
//! 在这里会以 `[delta-audit] ... 0E1BAC61(location(锚点)) removed` 当场拒绝。

use super::property_gate::{category_of, fingerprint};
use sc_properties::PropertyFile;
use std::collections::{BTreeMap, BTreeSet};

/// 审计结果（进阶段台账）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditReport {
    pub changed: usize,
    pub by_category: BTreeMap<String, Vec<String>>,
}

impl AuditReport {
    pub fn to_json(&self) -> serde_json::Value {
        let categories: serde_json::Value = self
            .by_category
            .iter()
            .map(|(category, columns)| (category.clone(), serde_json::json!(columns)))
            .collect();
        serde_json::json!({
            "changed": self.changed,
            "categories": categories,
        })
    }
}

/// 逐列对拍。`allowed` 是声明位的列 hash 集合（调用方组合）；`self_instance`
/// 为 Lot 自身实例（自引用归一，身份换代不算漂移）。
///
/// 返回 `Ok(None)` = 无漂移；`Ok(Some(report))` = 只有允许的漂移；
/// `Err` = 未声明漂移（构建必须中止）。
pub(crate) fn audit(
    reference: &PropertyFile,
    built: &PropertyFile,
    reference_self_instance: u32,
    built_self_instance: u32,
    allowed: &[u32],
    declared_families: &[String],
) -> Result<Option<AuditReport>, String> {
    let allowed: BTreeSet<u32> = allowed.iter().copied().collect();
    let before = fingerprint(reference, reference_self_instance);
    let after = fingerprint(built, built_self_instance);
    let mut by_category: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut violations = 0usize;

    for (hash, signature) in &after {
        let drift = match before.get(hash) {
            None => Some(format!("{hash:08X} added {}", signature.0)),
            Some(previous) if previous != signature => Some(format!(
                "{hash:08X} changed {}x{} -> {}x{}",
                previous.0,
                previous.1,
                signature.0,
                signature.1
            )),
            Some(_) => None,
        };
        if let Some(text) = drift {
            by_category
                .entry(category_of(*hash).to_string())
                .or_default()
                .push(text);
            if !allowed.contains(hash) {
                violations += 1;
            }
        }
    }
    for hash in before.keys() {
        if !after.contains_key(hash) {
            by_category
                .entry(category_of(*hash).to_string())
                .or_default()
                .push(format!("{hash:08X} removed"));
            if !allowed.contains(hash) {
                violations += 1;
            }
        }
    }

    if violations > 0 {
        let detail = serde_json::to_string(&by_category).unwrap_or_default();
        return Err(format!(
            "{violations} undeclared lot column(s) drifted from the reference lot — \
             场景声明 [{}] 之外的列被改动；构建已中止，上一次成功产物未被覆盖。\
             明细（列hash → 变化）: {detail}",
            declared_families.join(", ")
        ));
    }
    if by_category.is_empty() {
        return Ok(None);
    }
    Ok(Some(AuditReport {
        changed: by_category.values().map(Vec::len).sum(),
        by_category,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_properties::{PropType, Property, PropertyEncoding, Value};
    use std::collections::BTreeMap;

    const LIGHT_IDS: u32 = 0x0CAA_8F10;
    const PROP_ID_1: u32 = 0x0C12_EF21;
    const PATH_POINTS: u32 = 0x0CAA_680D;

    fn column(hash: u32, values: Vec<Value>) -> Property {
        Property {
            hash,
            prop_type: PropType::Vector3,
            kind: sc_properties::Kind::Array(values),
            encoding: PropertyEncoding::default(),
        }
    }

    fn file(columns: Vec<Property>) -> PropertyFile {
        PropertyFile {
            claimed_count: columns.len() as u32,
            values: columns,
        }
    }

    fn point(x: f32) -> Value {
        Value::Vector3([x, 0.0, 0.0])
    }

    /// 参照 lot：灯/锚点/路径三类列齐全。
    fn reference() -> PropertyFile {
        file(vec![
            column(LIGHT_IDS, vec![point(1.0)]),
            column(PROP_ID_1, vec![point(2.0)]),
            column(PATH_POINTS, vec![point(3.0)]),
        ])
    }

    #[test]
    fn preserves_undeclared_columns_green() {
        // 只声明 paths：灯/道具列原样保留 → 无漂移。
        let built = reference();
        let report = audit(&reference(), &built, 0, 0, &[PATH_POINTS], &["paths".into()]).unwrap();
        assert!(report.is_none());
    }

    #[test]
    fn flags_removed_light_and_names_category() {
        // r10 形态：整体重写丢掉了灯列。
        let built = file(vec![column(PATH_POINTS, vec![point(3.0)])]);
        let error = audit(&reference(), &built, 0, 0, &[PATH_POINTS], &["paths".into()]).unwrap_err();
        assert!(error.contains("light(灯)"), "{error}");
        assert!(error.contains("0CAA8F10"), "{error}");
        assert!(error.contains("removed"), "{error}");
        assert!(error.contains("构建已中止"), "{error}");
    }

    #[test]
    fn flags_prop_bin_drift_when_not_declared() {
        let built = file(vec![
            column(LIGHT_IDS, vec![point(1.0)]),
            column(PROP_ID_1, vec![point(9.0)]),
            column(PATH_POINTS, vec![point(3.0)]),
        ]);
        let error = audit(&reference(), &built, 0, 0, &[PATH_POINTS], &["paths".into()]).unwrap_err();
        assert!(error.contains("prop-bin"), "{error}");
    }

    #[test]
    fn allows_declared_column_changes() {
        let built = file(vec![
            column(LIGHT_IDS, vec![point(1.0)]),
            column(PROP_ID_1, vec![point(2.0)]),
            column(PATH_POINTS, vec![point(7.0)]),
        ]);
        let report = audit(&reference(), &built, 0, 0, &[PATH_POINTS], &["paths".into()])
            .unwrap()
            .expect("declared drift must be reported, not rejected");
        assert_eq!(report.changed, 1);
        assert_eq!(report.by_category["path(路网)"].len(), 1);
    }
}
