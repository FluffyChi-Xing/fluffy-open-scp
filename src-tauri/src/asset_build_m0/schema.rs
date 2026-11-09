use crate::asset_build_m0::diagnostics::{Diagnostic, DiagnosticSet};
use dbpf::ResourceId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

pub const ASSET_SCHEMA: &str = "openscp.asset";
pub const ASSET_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetSchemaV2 {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub kind: String,
    pub reference: ReferenceSchema,
    pub scene: SceneSchema,
    #[serde(default)]
    pub semantics: SemanticsSchema,
    pub outputs: OutputSchema,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceSchema {
    pub package: Option<String>,
    pub menu: Option<String>,
    pub unit: Option<String>,
    pub lot: Option<String>,
    #[serde(rename = "unitTemplate")]
    pub unit_template: Option<String>,
    #[serde(rename = "lotTemplate")]
    pub lot_template: Option<String>,
    /// Lot 来源预留接口（2026-11-08）：`"donor"`（缺省，克隆参照 lot）或
    /// `"drawn"`（自有绘制流）。绘制流未闭环前显式声明 "drawn" 会被
    /// fail-closed 拒绝——接口先占位，绘制流落地后在此分发。
    #[serde(default, rename = "lotSource")]
    pub lot_source: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSchema {
    pub document: String,
    /// delta 模式声明（schema v3，2026-11-07 事故后定案）：列出的列族才会
    /// 被场景写回覆盖（"paths" / "props:<bin>" / "lights" / "spawners" /
    /// "effects" / "decals"）；未声明的列从参照 lot **原样保留**。
    /// 缺省 = 权威模式（整体重写，仅限从零构建的 native profile）。
    #[serde(default)]
    pub writes: Vec<String>,
    pub model: Option<String>,
    /// RW4 编译形状。生产路径（2026-11-09）：缺省或显式只能是
    /// `"building40"`（SC5 Packager 算法的 Rust 内核）。`nh-atlas` /
    /// `merged-single` 已从生产入口移除。
    #[serde(default, rename = "modelShape")]
    pub model_shape: Option<String>,
    #[serde(default)]
    pub slots: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticsSchema {
    #[serde(rename = "class")]
    pub asset_class: Option<String>,
    #[serde(default)]
    pub unit_overrides: BTreeMap<String, Value>,
    #[serde(default)]
    pub lot_bindings: BTreeMap<String, Value>,
    #[serde(default)]
    pub menu: BTreeMap<String, Value>,
    /// `semantics.behavior`: 行为处理器声明（behavior.rs）。菜单/行为拆分：
    /// 行为绑定冻结 donor 身份，仅 Unit/Lot/locale(title) 增量。
    #[serde(default)]
    pub behavior: BTreeMap<String, Value>,
    /// `semantics.labor`: 建筑基本信息（W1/W2/W3 岗位 + 最低生产人数）。
    /// 逻辑编排图编译产物；字段缺失时保持旧 schema 的规范哈希不变。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labor: Option<Value>,
    /// `semantics.behaviors`: 逻辑编排图的行为节点数组（基础聚合 + 叶子）。
    /// 与 legacy `behavior` 互斥；两者并存时 plan 阶段拒绝。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub behaviors: Vec<Value>,
    /// Property patch documents are intentionally kept as JSON at M0. The
    /// typed Property writer owns their domain validation in M1, while M0
    /// still includes them in the schema hash and intermediate contract.
    #[serde(default)]
    pub properties: Vec<Value>,
    #[serde(default, rename = "allowSharedTemplate")]
    pub allow_shared_template: bool,
    /// G1 材质指纹闸门声明位（2026-11-05）：模型/六槽输入有意变更时置
    /// true，指纹漂移放行并刷新 `.build/material-baseline.json`；缺省
    /// false 时任何未声明的材质指纹漂移都会在 insert-model 失败。
    #[serde(default, rename = "materialChange")]
    pub material_change: bool,
    /// P3 属性验收闸门声明位（2026-11-06）：Asset/Unit 已验证列（材质/
    /// 地面/树/灯/效果/道具/路网/锚点）有意变更时置 true，属性闸门放行并
    /// 刷新 `.build/property-baseline.json`；缺省 false 时任何未声明的列
    /// 漂移都在 scene-semantics 失败并点名责任列。
    #[serde(default, rename = "lotChange")]
    pub lot_change: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputSchema {
    pub package: String,
    /// Package write strategy: `"delta"` (default — only the resources this
    /// build creates or rewrites) or `"full"` (clone of the reference
    /// package; legacy overlay contract).
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSchema {
    pub value: AssetSchemaV2,
    pub canonical_json: Vec<u8>,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaError {
    pub diagnostics: DiagnosticSet,
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "asset schema validation failed")
    }
}

impl std::error::Error for SchemaError {}

pub fn parse_schema(input: &str) -> Result<ParsedSchema, SchemaError> {
    let value = match serde_json::from_str::<AssetSchemaV2>(input) {
        Ok(value) => value,
        Err(error) => {
            let mut diagnostics = DiagnosticSet::default();
            diagnostics.push(Diagnostic::error(
                "schema.invalid_json",
                None,
                error.to_string(),
            ));
            return Err(SchemaError { diagnostics });
        }
    };
    let mut diagnostics = DiagnosticSet::default();
    validate_schema(&value, &mut diagnostics);
    if diagnostics.has_errors() {
        return Err(SchemaError { diagnostics });
    }
    let canonical_json = serde_json::to_vec(&value).map_err(|error| SchemaError {
        diagnostics: DiagnosticSet {
            items: vec![Diagnostic::error(
                "schema.serialize_failed",
                None,
                error.to_string(),
            )],
        },
    })?;
    let hash = format!("{:x}", Sha256::digest(&canonical_json));
    Ok(ParsedSchema {
        value,
        canonical_json,
        hash,
    })
}

pub fn validate_schema(schema: &AssetSchemaV2, diagnostics: &mut DiagnosticSet) {
    if schema.schema != ASSET_SCHEMA {
        diagnostics.push(Diagnostic::error(
            "schema.unsupported_format",
            Some("$schema".into()),
            format!("expected {ASSET_SCHEMA}"),
        ));
    }
    if schema.schema_version != ASSET_SCHEMA_VERSION {
        diagnostics.push(Diagnostic::error(
            "schema.unsupported_version",
            Some("schema_version".into()),
            format!("expected {ASSET_SCHEMA_VERSION}"),
        ));
    }
    if !valid_identifier(&schema.id) {
        diagnostics.push(Diagnostic::error(
            "schema.invalid_id",
            Some("id".into()),
            "id must contain 1-128 ASCII letters, digits, '_' or '-'",
        ));
    }
    if schema.kind != "building-asset" {
        diagnostics.push(Diagnostic::error(
            "schema.unsupported_kind",
            Some("kind".into()),
            "M0 accepts kind building-asset",
        ));
    }
    if schema.reference.lot_source.as_deref() == Some("drawn") {
        diagnostics.push(Diagnostic::warning(
            "schema.lotSourceDrawnReserved",
            Some("reference.lotSource".into()),
            "自有绘制流接口已预留、尚未实现；当前请使用 donor 参照（缺省）",
        ));
    }
    if let Some(package) = &schema.reference.package {
        validate_relative_file(package, "reference.package", diagnostics);
    }
    validate_relative_file(&schema.scene.document, "scene.document", diagnostics);
    if let Some(model) = &schema.scene.model {
        validate_relative_file(model, "scene.model", diagnostics);
    }
    for (slot, input) in &schema.scene.slots {
        if !valid_slot(slot) {
            diagnostics.push(Diagnostic::error(
                "schema.invalid_slot",
                Some(format!("scene.slots.{slot}")),
                "slots must use slot0 through slot5",
            ));
        }
        validate_relative_file(input, &format!("scene.slots.{slot}"), diagnostics);
    }
    if schema.outputs.package.is_empty()
        || !schema.outputs.package.ends_with(".package")
        || schema.outputs.package.contains(['/', '\\', ':'])
    {
        diagnostics.push(Diagnostic::error(
            "schema.invalid_output",
            Some("outputs.package".into()),
            "package must be a filename ending in .package",
        ));
    }
    if let Some(shape) = &schema.scene.model_shape {
        // 2026-11-09：生产路径只允许 Building40（Rust 迁入的 Packager 建筑核）。
        if shape != "building40" {
            diagnostics.push(Diagnostic::error(
                "schema.invalid_model_shape",
                Some("scene.modelShape".into()),
                r#"modelShape must be "building40" (or omitted). Production builds use the Rust Building40 kernel; nh-atlas/merged-single and mesh-splitting routes are removed. Do not shell out to SC5 Asset Packager."#,
            ));
        }
    }
    reject_external_packager_fields(schema, diagnostics);
    if let Some(mode) = &schema.outputs.mode {
        if mode != "delta" && mode != "full" {
            diagnostics.push(Diagnostic::error(
                "schema.invalid_output_mode",
                Some("outputs.mode".into()),
                r#"mode must be "delta" (only resources this build creates or rewrites) or "full" (reference package clone)"#,
            ));
        }
    }
    for (field, value) in [
        ("reference.menu", &schema.reference.menu),
        ("reference.unit", &schema.reference.unit),
        ("reference.lot", &schema.reference.lot),
        ("reference.unitTemplate", &schema.reference.unit_template),
        ("reference.lotTemplate", &schema.reference.lot_template),
    ] {
        if let Some(value) = value {
            if parse_tgi(value).is_err() {
                diagnostics.push(Diagnostic::error(
                    "schema.invalid_tgi",
                    Some(field.into()),
                    "TGI must be TYPE:GROUP:INSTANCE in hexadecimal",
                ));
            }
        }
    }
}

pub fn parse_tgi(value: &str) -> Result<ResourceId, String> {
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != 3 {
        return Err("TGI must contain three components".into());
    }
    let mut numbers = [0u32; 3];
    for (index, part) in parts.iter().enumerate() {
        let text = part.trim().trim_start_matches("0x");
        numbers[index] = u32::from_str_radix(text, 16)
            .map_err(|_| format!("invalid hexadecimal component {part}"))?;
    }
    Ok(ResourceId {
        type_id: numbers[0],
        group: numbers[1],
        instance: numbers[2],
    })
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn valid_slot(value: &str) -> bool {
    matches!(
        value,
        "slot0" | "slot1" | "slot2" | "slot3" | "slot4" | "slot5"
    )
}

/// Fail-closed against schemas that still try to drive the Python SC5
/// Asset Packager (`.pyc` / `backend.command` / external packager keys).
/// Production packaging is Rust Building40 only (2026-11-09).
fn reject_external_packager_fields(schema: &AssetSchemaV2, diagnostics: &mut DiagnosticSet) {
    let blob = serde_json::to_string(schema).unwrap_or_default();
    let lower = blob.to_ascii_lowercase();
    let markers = [
        ("sc5_packager", "sc5_packager"),
        ("backend.command", "backend.command"),
        ("\"backend\"", "backend"),
        (".pyc", ".pyc"),
        ("externalpackager", "externalPackager"),
        ("packagercommand", "packagerCommand"),
    ];
    for (needle, label) in markers {
        if lower.contains(needle) {
            diagnostics.push(Diagnostic::error(
                "schema.external_packager_forbidden",
                Some(label.into()),
                "external SC5 Asset Packager / Python .pyc backends are forbidden; use the Rust asset_build_m1 Building40 path (build_queue_enqueue_project)",
            ));
            break;
        }
    }
}

fn validate_relative_file(value: &str, path: &str, diagnostics: &mut DiagnosticSet) {
    let candidate = Path::new(value);
    let invalid = value.is_empty()
        || candidate.is_absolute()
        || candidate
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir));
    if invalid {
        diagnostics.push(Diagnostic::error(
            "schema.invalid_path",
            Some(path.into()),
            "path must be a non-empty project-relative file",
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> String {
        serde_json::json!({
            "$schema": "openscp.asset",
            "schema_version": 2,
            "id": "willis-tower",
            "kind": "building-asset",
            "reference": {
                "menu": "00B1B104:09878A01:74168A83",
                "unit": "00B1B104:40E0C000:8D007745",
                "lot": "00B1B104:58F0C000:8D007745"
            },
            "scene": {
                "document": "asset.lot.json",
                "model": "assets/models/willis.lotm",
                "slots": {"slot0": "schemas/assets/texture-input-1.json"}
            },
            "semantics": {"class": "landmark"},
            "outputs": {"package": "willis.package"}
        })
        .to_string()
    }

    #[test]
    fn parses_and_hashes_a_schema_deterministically() {
        let parsed = parse_schema(&schema()).unwrap();
        assert_eq!(parsed.value.id, "willis-tower");
        assert_eq!(parsed.hash.len(), 64);
        assert_eq!(parsed.hash, parse_schema(&schema()).unwrap().hash);
    }

    #[test]
    fn rejects_absolute_paths_and_instance_only_tgis() {
        let mut value: Value = serde_json::from_str(&schema()).unwrap();
        value["scene"]["document"] = Value::String("C:\\asset.lot.json".into());
        value["reference"]["unit"] = Value::String("8D007745".into());
        let error = parse_schema(&value.to_string()).unwrap_err();
        let codes: Vec<_> = error
            .diagnostics
            .items
            .iter()
            .map(|item| item.code.as_str())
            .collect();
        assert!(codes.contains(&"schema.invalid_path"));
        assert!(codes.contains(&"schema.invalid_tgi"));
    }

    #[test]
    fn accepts_building40_or_omitted_model_shape() {
        let mut value: Value = serde_json::from_str(&schema()).unwrap();
        assert!(parse_schema(&value.to_string()).is_ok());
        value["scene"]["modelShape"] = Value::String("building40".into());
        assert!(parse_schema(&value.to_string()).is_ok());
    }

    #[test]
    fn rejects_legacy_model_shapes_and_packager_markers() {
        let mut value: Value = serde_json::from_str(&schema()).unwrap();
        value["scene"]["modelShape"] = Value::String("nh-atlas".into());
        let error = parse_schema(&value.to_string()).unwrap_err();
        assert!(error
            .diagnostics
            .items
            .iter()
            .any(|item| item.code == "schema.invalid_model_shape"));

        value["scene"]["modelShape"] = Value::String("merged-single".into());
        let error = parse_schema(&value.to_string()).unwrap_err();
        assert!(error
            .diagnostics
            .items
            .iter()
            .any(|item| item.code == "schema.invalid_model_shape"));

        let mut packager = serde_json::from_str::<Value>(&schema()).unwrap();
        packager["id"] = Value::String("demo_sc5_packager_bridge".into());
        let error = parse_schema(&packager.to_string()).unwrap_err();
        assert!(
            error
                .diagnostics
                .items
                .iter()
                .any(|item| item.code == "schema.external_packager_forbidden"),
            "{error:?}"
        );
    }
}
