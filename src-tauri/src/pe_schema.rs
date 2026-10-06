//! PE schema JSON 引擎（低代码资产管线 v1，docs/design/lowcode-asset-pipeline.md）：
//! 把 Property Editor 的当前状态（session + 前端编辑层覆盖）规范化为
//! `openscp.lot-asset/1` schema JSON。
//!
//! 架构：编辑态由前端持有（transform/字段覆盖、图层可见性），schema 格式由
//! 本模块持有（单一真源）——前端提交当前 session DTO + 编辑覆盖，引擎产出
//! 规范化 JSON（hex TGI、版本字段、字段序即文档序）。未来写回器（.package
//! 打包）消费同一 JSON，解析引擎按 version 分发。

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// TGI → 十六进制字符串对象（文档 §3：域内惯例，解析器同时接受十进制）。
fn tgi_hex(tgi: &TgiInput) -> Value {
    json!({
        "typeId": format!("0x{:08X}", tgi.type_id),
        "group": format!("0x{:08X}", tgi.group),
        "instance": format!("0x{:08X}", tgi.instance),
    })
}

fn opt_tgi_hex(tgi: &Option<TgiInput>) -> Value {
    tgi.as_ref().map(tgi_hex).unwrap_or(Value::Null)
}

fn f32_array(values: &[serde_json::Value]) -> Vec<f64> {
    values.iter().filter_map(|v| v.as_f64()).collect()
}

/// 前端 `Tgi`（十进制 u32）镜像。
#[derive(Deserialize)]
pub struct TgiInput {
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
}

/// 前端 `LotEditorSession` 的 schema 相关子集（serde 忽略未列字段）。
#[derive(Deserialize)]
pub struct SessionInput {
    #[serde(default)]
    pub asset_name: Option<String>,
    pub tgi: TgiInput,
    #[serde(default)]
    pub model_lods: Vec<Option<LodRefInput>>,
    #[serde(default)]
    pub lot_size: Option<[f64; 2]>,
    #[serde(default)]
    pub lot_tile_period: Option<[f64; 2]>,
    #[serde(default)]
    pub lot_placement: Option<Vec<f64>>,
    #[serde(default)]
    pub lot_colors: Vec<[f64; 4]>,
    #[serde(default)]
    pub lot_colors_authored: Vec<bool>,
    #[serde(default)]
    pub lot_border_colors: Vec<[f64; 3]>,
    #[serde(default)]
    pub lot_border_widths: Vec<f64>,
    #[serde(default)]
    pub lot_border_pattern_indices: Vec<i64>,
    #[serde(default)]
    pub lot_base_tile: Option<u8>,
    #[serde(default)]
    pub lot_overlay_box_offset: Option<[f64; 2]>,
    #[serde(default)]
    pub lot_model_bbox_center: Option<[f64; 2]>,
    #[serde(default)]
    pub units: Vec<UnitInput>,
}

#[derive(Deserialize)]
pub struct LodRefInput {
    pub tgi: TgiInput,
}

/// 前端 `LotUnitDto` 镜像：公共字段 + kind 特有字段整体保留（tagged 展开
/// 在 unit_json 里按 kind 映射，未升格字段原样透传）。
#[derive(Deserialize)]
pub struct UnitInput {
    pub kind: String,
    #[serde(default)]
    pub index: i64,
    #[serde(default)]
    pub bin: Option<i64>,
    #[serde(default)]
    pub category: Option<i64>,
    #[serde(default)]
    pub transform: Option<TransformInput>,
    /// kind 特有字段整包（light/color/outerRadius/…按 kind 取用）。
    #[serde(flatten)]
    pub extra: Map<String, Value>,
    /// 属性行（hash+typeName+value 字符串）——schema 的 fields 透传源。
    #[serde(default)]
    pub fields: Vec<FieldInput>,
}

#[derive(Deserialize)]
pub struct TransformInput {
    /// WPF Matrix3D 行主序 12 floats。
    #[serde(default)]
    pub matrix: Vec<f64>,
}

#[derive(Deserialize)]
pub struct FieldInput {
    pub hash: u32,
    #[serde(default)]
    pub type_name: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

/// 前端编辑层覆盖（unitEditLayer）。
#[derive(Deserialize)]
pub struct UnitEditInput {
    pub id: String,
    /// 行主序 12 floats（transform 整体替换）。
    #[serde(default)]
    pub matrix: Option<Vec<f64>>,
    /// 属性字段 patch（hash hex 字符串键 → 值），并入 fields。
    #[serde(default)]
    pub fields: Option<BTreeMap<String, Value>>,
}

#[derive(Deserialize)]
pub struct BuildPeSchemaRequest {
    pub session: SessionInput,
    /// 编辑层：unitId → 覆盖。
    #[serde(default)]
    pub overrides: Vec<UnitEditInput>,
    /// 隐藏的 unit id（编辑器可见性，入 editor 节）。
    #[serde(default)]
    pub hidden_unit_ids: Vec<String>,
    /// 图层可见性（编辑器视图状态）。
    #[serde(default)]
    pub groups: BTreeMap<String, bool>,
}

fn unit_id(kind: &str, bin: Option<i64>, category: Option<i64>, index: i64) -> String {
    match kind {
        "prop" => format!("prop:{}:{}", bin.unwrap_or(0), index),
        "decal" => format!("decal:{}:{}", category.unwrap_or(0), index),
        _ => format!("{}:{}", kind, index),
    }
}

/// kind → schema 单元对象（文档 §7 字段映射；fields 透传位含属性行）。
fn unit_json(
    unit: &UnitInput,
    id: &str,
    override_edit: Option<&UnitEditInput>,
    hidden: bool,
) -> Value {
    let mut object = Map::new();
    object.insert("id".into(), json!(id));
    object.insert("kind".into(), json!(unit.kind));
    object.insert("visible".into(), json!(!hidden));
    // 变换：覆盖矩阵优先（行主序 12 floats）
    let matrix = override_edit
        .and_then(|edit| edit.matrix.as_ref())
        .or_else(|| unit.transform.as_ref().map(|t| &t.matrix));
    if let Some(matrix) = matrix {
        object.insert(
            "transform".into(),
            json!({"matrix": matrix, "flags": 15}),
        );
    }
    // kind 特有字段（tauri.ts LotUnitDto 逐字映射）
    let extra = &unit.extra;
    match unit.kind.as_str() {
        "light" => {
            for key in [
                "lightType",
                "color",
                "outerRadius",
                "innerRadius",
                "diffuse",
                "length",
                "cullDistance",
                "isVolumetric",
                "debugName",
            ] {
                if let Some(value) = extra.get(key) {
                    object.insert(key.to_string(), value.clone());
                }
            }
        }
        "effect" => {
            if let Some(value) = extra.get("effectId") {
                object.insert("effectId".into(), value.clone());
            }
            if let Some(value) = extra.get("enabled") {
                object.insert("enabled".into(), value.clone());
            }
        }
        "decal" => {
            for key in ["category", "scale", "depth", "materialData"] {
                if let Some(value) = extra.get(key) {
                    object.insert(key.to_string(), value.clone());
                }
            }
        }
        "prop" => {
            for key in ["bin", "slot", "resourceId", "scale"] {
                if let Some(value) = extra.get(key) {
                    object.insert(key.to_string(), value.clone());
                }
            }
        }
        "pathPoint" => {
            for key in ["point", "tangent", "pointIndex"] {
                if let Some(value) = extra.get(key) {
                    object.insert(key.to_string(), value.clone());
                }
            }
        }
        "spawner" => {
            for key in ["id", "count", "countRandom", "agent"] {
                if let Some(value) = extra.get(key) {
                    object.insert(key.to_string(), value.clone());
                }
            }
        }
        _ => {}
    }
    // fields 透传位：属性行（hash hex → value 字符串原样；写回器按
    // typeName 反编码）+ 编辑层字段 patch 并入（patch 优先）。
    let mut fields = Map::new();
    for field in &unit.fields {
        fields.insert(
            format!("0x{:08X}", field.hash),
            json!(field.value.clone().unwrap_or_default()),
        );
    }
    if let Some(edit) = override_edit {
        if let Some(patch) = &edit.fields {
            for (key, value) in patch {
                fields.insert(key.clone(), value.clone());
            }
        }
    }
    if !fields.is_empty() {
        object.insert("fields".into(), Value::Object(fields));
    }
    Value::Object(object)
}

/// 组装 `openscp.lot-asset/1` schema（docs/design/lowcode-asset-pipeline.md §4）。
pub fn build_schema(request: &BuildPeSchemaRequest) -> String {
    let session = &request.session;
    let mut overrides: BTreeMap<String, &UnitEditInput> = BTreeMap::new();
    for edit in &request.overrides {
        overrides.insert(edit.id.clone(), edit);
    }
    let hidden: std::collections::HashSet<&String> = request.hidden_unit_ids.iter().collect();

    let units: Vec<Value> = session
        .units
        .iter()
        .map(|unit| {
            let id = unit_id(&unit.kind, unit.bin, unit.category, unit.index);
            let hidden = hidden.contains(&id);
            unit_json(unit, &id, overrides.get(&id).copied(), hidden)
        })
        .collect();

    let lods: Vec<Value> = session
        .model_lods
        .iter()
        .enumerate()
        .map(|(index, lod)| {
            json!({
                "lod": index + 1,
                "ref": lod.as_ref().map(|l| json!({
                    "kind": "tgi",
                    "tgi": tgi_hex(&l.tgi),
                })).unwrap_or(Value::Null),
            })
        })
        .collect();

    let mut schema = Map::new();
    schema.insert("$schema".into(), json!("openscp.lot-asset/1"));
    schema.insert("version".into(), json!(1));
    schema.insert(
        "meta".into(),
        json!({
            "name": session.asset_name,
            "source": {"tgi": tgi_hex(&session.tgi)},
        }),
    );
    schema.insert(
        "lot".into(),
        json!({
            "size": session.lot_size,
            "tilePeriod": session.lot_tile_period,
            "placement": session.lot_placement,
            "baseTile": session.lot_base_tile,
            "colors": session.lot_colors.iter().enumerate().map(|(i, rgba)| json!({
                "rgba": rgba,
                "authored": session.lot_colors_authored.get(i).copied().unwrap_or(false),
            })).collect::<Vec<_>>(),
            "borderColors": session.lot_border_colors,
            "borderWidths": session.lot_border_widths,
            "borderPatterns": session.lot_border_pattern_indices,
            "overlayBoxOffset": session.lot_overlay_box_offset,
            "modelBBoxCenter": session.lot_model_bbox_center,
            "mask": {"kind": "source"},
        }),
    );
    schema.insert("model".into(), json!({ "lods": lods }));
    schema.insert("units".into(), json!(units));
    schema.insert(
        "editor".into(),
        json!({
            "groups": request.groups,
            "unitVisibility": request.hidden_unit_ids.iter().map(|id| (id.clone(), json!(false)))
                .collect::<BTreeMap<_, _>>(),
        }),
    );
    // 键序 = 文档序（serde_json 默认 BTreeMap 有序；Map 按插入序保留 feature）
    let value = Value::Object(schema);
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into())
}

/// 引擎命令：前端提交当前 session + 编辑层覆盖，返回规范化 schema JSON。
/// 无状态纯函数（格式引擎）；导出/持久化由前端与未来构建管线负责。
#[tauri::command]
pub fn build_pe_schema(
    request: BuildPeSchemaRequest,
) -> Result<PeSchemaResponse, crate::activity::CommandError> {
    use crate::activity::CommandError;
    let schema_json = build_schema(&request);
    Ok(PeSchemaResponse { schema_json })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeSchemaResponse {
    pub schema_json: String,
}
