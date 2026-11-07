//! P3 属性验收闸门（2026-11-06，死命令：已验证输出不允许回归）。
//!
//! Asset/Unit 逐列指纹（属性 hash → 类型/元素数/值摘要）与
//! `.build/property-baseline.json` 对拍：**未声明的漂移 = 构建失败**，错误
//! 文本点名 `[scene-semantics]` + 特征分类（prop-bin/light/path/ground/
//! location/board）+ old→new 摘要。材质合成、地面合成、树、灯、effect、
//! 道具、路网、锚点等已验证列全部被覆盖——任何处理器改坏它们，构建当场
//! 拒绝，不再需要实机截图或字节对拍。
//!
//! 自引用 instance（标题文本表、self Key、self 链接）按 0 归一，跨构建
//! 身份稳定；故意变更必须 schema 声明 `semantics.lotChange: true`，放行并
//! 刷新基线（与 G1 材质闸门 `materialChange` 同款契约）。

use sc_properties::{Kind, PropertyEncoding, PropertyFile, Value};
use std::collections::BTreeMap;
use std::path::Path;

const BASELINE_FILE: &str = ".build/property-baseline.json";

/// 列签名：属性类型、元素总数、归一化值摘要。
pub(crate) type ColSig = (String, usize, u64);

/// 已验证特征分类（错误消息可读性；未知列归 other，不改变闸门语义）。
pub(crate) fn category_of(hash: u32) -> &'static str {
    match hash {
        0x0CAA_680D | 0x0CB0_0ED8 | 0x0CAA_6832 | 0x0CAA_6841 | 0x0CC8_FE7A | 0x0D9A_4E54
        | 0x0D9A_A2BB => "path(路网)",
        0x0E1B_AC61 | 0x0E1B_AC62 => "location(锚点)",
        0x0CCB_7FC8..=0x0CCB_7FD6 | 0x0FE0_1484 | 0x00F9_EFBA | 0x0A14_EDB3..=0x0A14_EDB7
        | 0x0EC7_4232 | 0x0BC5_051B => "ground(地面)",
        0x0C12_EF20..=0x0C12_EF6C | 0x0DA7_CAD4..=0x0DA7_CB07 => "prop-bin(道具/树/车)",
        0x0CAA_8F10..=0x0CAA_8F17 | 0x0E01_200F | 0x0D4C_96B4 | 0x0D4C_AD23 | 0x0E4D_F244
        | 0x0EC4_567E | 0x0E98_D445 => "light(灯)",
        0x0D79_70ED => "board(城市看板)",
        0x00F9_EFBB..=0x00F9_EFBE | 0x0D00_82F1 | 0x0D00_82F3 | 0x0CAB_B2CF => "model-refs(模型引用)",
        _ => "other",
    }
}

/// 归一化值摘要：self instance 一律按 0 参与摘要，身份换代不产生漂移。
fn hash_value(value: &Value, self_instance: u32, state: &mut u64) {
    fn mix(state: &mut u64, bytes: &[u8]) {
        for byte in bytes {
            *state ^= u64::from(*byte);
            *state = state.wrapping_mul(0x0100_0000_01B3);
        }
        *state ^= 0xFF;
    }
    match value {
        Value::Bool(v) => mix(state, if *v { b"b1" } else { b"b0" }),
        Value::Int32(v) => mix(state, &v.to_le_bytes()),
        Value::UInt32(v) => mix(state, &v.to_le_bytes()),
        Value::Float(v) => mix(state, &v.to_bits().to_le_bytes()),
        Value::Key(k) => {
            let instance = if k.instance == self_instance { 0 } else { k.instance };
            mix(state, &k.type_id.to_le_bytes());
            mix(state, &k.group.to_le_bytes());
            mix(state, &instance.to_le_bytes());
        }
        Value::Text(t) => {
            let table = if t.table_id == self_instance { 0 } else { t.table_id };
            mix(state, &table.to_le_bytes());
            mix(state, &t.instance_id.to_le_bytes());
        }
        Value::String8(s) => mix(state, s.as_bytes()),
        Value::String16(s) => mix(state, s.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<u8>>().as_slice()),
        Value::ColorRgb { r, g, b } => {
            mix(state, &r.to_bits().to_le_bytes());
            mix(state, &g.to_bits().to_le_bytes());
            mix(state, &b.to_bits().to_le_bytes());
        }
        Value::ColorRgba { r, g, b, a } => {
            mix(state, &r.to_bits().to_le_bytes());
            mix(state, &g.to_bits().to_le_bytes());
            mix(state, &b.to_bits().to_le_bytes());
            mix(state, &a.to_bits().to_le_bytes());
        }
        Value::Vector2(v) => v.iter().for_each(|f| mix(state, &f.to_bits().to_le_bytes())),
        Value::Vector3(v) => v.iter().for_each(|f| mix(state, &f.to_bits().to_le_bytes())),
        Value::Vector4(v) => v.iter().for_each(|f| mix(state, &f.to_bits().to_le_bytes())),
        Value::BoundingBox { min, max } => {
            min.iter().for_each(|f| mix(state, &f.to_bits().to_le_bytes()));
            max.iter().for_each(|f| mix(state, &f.to_bits().to_le_bytes()));
        }
        Value::Transform(t) => {
            for f in &t.matrix {
                mix(state, &f.to_bits().to_le_bytes());
            }
        }
    }
}

fn kind_name(kind: &Kind) -> String {
    match kind {
        Kind::Scalar(_) => "scalar".into(),
        Kind::Array(values) => format!("array[{}]", values.len()),
        Kind::Empty => "empty".into(),
        other => format!("{other:?}"),
    }
}

pub(crate) fn fingerprint(file: &PropertyFile, self_instance: u32) -> BTreeMap<u32, ColSig> {
    let mut out: BTreeMap<u32, ColSig> = BTreeMap::new();
    for property in &file.values {
        let mut state = 0x811C_9DC5u64;
        let mut count = 0usize;
        match &property.kind {
            Kind::Scalar(value) => {
                hash_value(value, self_instance, &mut state);
                count = 1;
            }
            Kind::Empty => {}
            Kind::Array(values) => {
                for value in values {
                    hash_value(value, self_instance, &mut state);
                    count += 1;
                }
            }
        }
        let entry = (kind_name(&property.kind), count, state);
        out
            .entry(property.hash)
            .and_modify(|existing| {
                existing.1 += entry.1;
                existing.2 = existing.2.wrapping_add(entry.2).rotate_left(17);
            })
            .or_insert(entry);
    }
    out
}

pub(crate) fn fingerprint_to_json(fp: &BTreeMap<u32, ColSig>) -> serde_json::Value {
    serde_json::Value::Object(
        fp.iter()
            .map(|(hash, (t, n, d))| {
                (
                    format!("{hash:08X}"),
                    serde_json::json!([t, n, format!("{d:016X}")]),
                )
            })
            .collect(),
    )
}

fn fingerprint_from_json(value: &serde_json::Value) -> BTreeMap<u32, ColSig> {
    value
        .as_object()
        .map(|map| {
            map.iter()
                .filter_map(|(hash, sig)| {
                    let hash = u32::from_str_radix(hash.trim_start_matches("0x"), 16).ok()?;
                    let sig = sig.as_array()?;
                    Some((
                        hash,
                        (
                            sig.first()?.as_str()?.to_string(),
                            sig.get(1)?.as_u64()? as usize,
                            u64::from_str_radix(sig.get(2)?.as_str()?.trim_start_matches("0x"), 16)
                                .ok()?,
                        ),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 闸门入口：`asset`/`unit` 为本次构建的成品属性表，`self_instance` 为建筑
/// 身份。基线缺失 = 建立基线（首次验收）；漂移非空且未声明 = Err（点名）。
pub fn gate(
    root: &Path,
    asset: &PropertyFile,
    unit: &PropertyFile,
    self_instance: u32,
    declared_change: bool,
) -> Result<serde_json::Value, String> {
    let asset_fp = fingerprint(asset, self_instance);
    let unit_fp = fingerprint(unit, self_instance);
    let baseline_path = root.join(BASELINE_FILE);
    let baseline = std::fs::read(&baseline_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let Some(baseline) = baseline else {
        let established = serde_json::json!({
            "asset": fingerprint_to_json(&asset_fp),
            "unit": fingerprint_to_json(&unit_fp),
        });
        store_baseline(&baseline_path, &established)?;
        return Ok(serde_json::json!({"baseline": "established"}));
    };
    let baseline_asset = fingerprint_from_json(baseline.get("asset").unwrap_or(&serde_json::Value::Null));
    let baseline_unit = fingerprint_from_json(baseline.get("unit").unwrap_or(&serde_json::Value::Null));

    let mut report = serde_json::json!({"baseline": "compared"});
    let mut drift_total = 0usize;
    for (side, built, old) in
        [("asset", &asset_fp, &baseline_asset), ("unit", &unit_fp, &baseline_unit)]
    {
        let mut changed = Vec::new();
        let mut added = Vec::new();
        let mut removed = Vec::new();
        for (hash, sig) in built {
            match old.get(hash) {
                None => added.push(format!("{:08X}({})", hash, category_of(*hash))),
                Some(prev) if prev != sig => changed.push(format!(
                    "{:08X}({}) {}x{:016X} -> {}x{:016X}",
                    hash,
                    category_of(*hash),
                    u32_tuple_str(prev),
                    prev.2,
                    u32_tuple_str(sig),
                    sig.2
                )),
                Some(_) => {}
            }
        }
        for hash in old.keys() {
            if !built.contains_key(hash) {
                removed.push(format!("{:08X}({})", hash, category_of(*hash)));
            }
        }
        drift_total += changed.len() + added.len() + removed.len();
        if !changed.is_empty() || !added.is_empty() || !removed.is_empty() {
            report[side] = serde_json::json!({"changed": changed, "added": added, "removed": removed});
        }
    }
    if drift_total == 0 {
        return Ok(report);
    }
    if declared_change {
        let established = serde_json::json!({
            "asset": fingerprint_to_json(&asset_fp),
            "unit": fingerprint_to_json(&unit_fp),
        });
        store_baseline(&baseline_path, &established)?;
        report["declared"] = serde_json::Value::Bool(true);
        return Ok(report);
    }
    let summary = serde_json::to_string(&report).unwrap_or_default();
    Err(format!(
        "[scene-semantics] property gate: {drift_total} verified column drift(s) vs accepted \
         baseline — 已验证输出（材质/地面/树/灯/效果/道具/路网/锚点）被未声明地改动。\
         有意变更请在 schema 声明 `semantics.lotChange: true`；漂移明细: {summary}"
    ))
}

fn u32_tuple_str(sig: &ColSig) -> String {
    format!("{}x{}", sig.1, sig.0)
}

fn store_baseline(path: &Path, baseline: &serde_json::Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("property gate dir: {error}"))?;
    }
    let bytes = serde_json::to_vec_pretty(baseline).map_err(|error| format!("property gate encode: {error}"))?;
    std::fs::write(path, bytes).map_err(|error| format!("property gate write: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_properties::{PropType, Property};

    fn file_with(hash: u32, value: Value) -> PropertyFile {
        PropertyFile {
            values: vec![Property {
                hash,
                prop_type: PropType::Bool,
                kind: Kind::Scalar(value),
                encoding: PropertyEncoding::default(),
            }],
            claimed_count: 1,
        }
    }

    #[test]
    fn self_instance_normalization_keeps_identity_stable() {
        let a = file_with(0x08E1_7ED7, Value::Key(sc_properties::Key { type_id: 0, group: 0x40E1_C000, instance: 0xAAAA }));
        let b = file_with(0x08E1_7ED7, Value::Key(sc_properties::Key { type_id: 0, group: 0x40E1_C000, instance: 0xBBBB }));
        let fa = fingerprint(&a, 0xAAAA);
        let fb = fingerprint(&b, 0xBBBB);
        assert_eq!(fa[&0x08E1_7ED7].2, fb[&0x08E1_7ED7].2, "self-ref must normalize");
        let fc = fingerprint(&b, 0xCCCC);
        assert_ne!(fb[&0x08E1_7ED7].2, fc[&0x08E1_7ED7].2, "non-self ref must count as drift");
    }

    #[test]
    fn gate_establishes_then_flags_undeclared_drift() {
        // 沙箱环境常拒绝 $TMP 写入，与 tests::temp_root 同款：优先工作区
        // target/test-tmp，系统临时目录兜底。
        let workspace_target = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-tmp");
        let base = if std::fs::create_dir_all(&workspace_target).is_ok() {
            workspace_target
        } else {
            std::env::temp_dir()
        };
        let root = base.join(format!(
            "openscp-gate-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|value| value.as_millis())
                .unwrap_or_default()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let asset = file_with(0x0C12_EF2C, Value::Bool(true));
        let unit = file_with(0x0AD7_FAC9, Value::Float(250.0));
        let first = gate(&root, &asset, &unit, 0x1, false).unwrap();
        assert_eq!(first["baseline"], "established");
        // 相同内容二构建：无漂移。
        let second = gate(&root, &asset, &unit, 0x1, false).unwrap();
        assert_eq!(second["baseline"], "compared");
        // 未声明漂移：树列被改 → 失败并点名分类。
        let drifted = file_with(0x0C12_EF2C, Value::Bool(false));
        let error = gate(&root, &drifted, &unit, 0x1, false).unwrap_err();
        assert!(error.contains("prop-bin"), "error must name the category: {error}");
        assert!(error.contains("0C12EF2C"), "error must name the column: {error}");
        // 声明漂移：放行并刷新基线。
        gate(&root, &drifted, &unit, 0x1, true).unwrap();
        let after = gate(&root, &drifted, &unit, 0x1, false).unwrap();
        assert_eq!(after["baseline"], "compared");
        let _ = std::fs::remove_dir_all(&root);
    }
}
