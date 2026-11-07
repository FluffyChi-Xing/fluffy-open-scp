//! Scene document (openscp.lot-asset/1) → Lot property unit columns.
//!
//! The workbench's manual edits — placed props with bins, path entities and
//! their points, lights, spawners, effects — live in `asset.lot.json`.  The
//! v1 builder never lowered them: the built package kept the referenced lot's
//! original unit columns (its props/lights/paths came back) and the user's
//! edits vanished (user report 2026-10-30, 问题2/问题3).  This module is the
//! write side of `sc_properties::lot_unit::assemble_units`: it strips **all**
//! original unit columns from the cloned Lot property and writes the scene's
//! units in the same parallel-array convention (unit #N = index N in every
//! column; a column ends at the first absent field).

use dbpf::ResourceId;
use sc_properties::lot_unit as unit_hash;
use sc_properties::{
    Key, Kind, LotEditorDocument, PropType, Property, PropertyEncoding, PropertyFile, Transform,
    Value,
};
use serde_json::Value as Json;

/// All unit column hashes: the writeback replaces the complete unit set, so a
/// referenced lot's own props/lights/paths never leak into the built asset.
pub fn unit_column_hashes() -> Vec<u32> {
    let mut hashes = Vec::new();
    hashes.extend_from_slice(&[
        unit_hash::LIGHT_IDS,
        unit_hash::LIGHT_TYPES,
        unit_hash::LIGHT_TRANSFORMS,
        unit_hash::LIGHT_COLORS,
        unit_hash::LIGHT_RADII,
        unit_hash::LIGHT_INNER_RADII,
        unit_hash::LIGHT_DIFFUSE_LEVELS,
        unit_hash::LIGHT_DEBUG_NAMES,
        unit_hash::LIGHT_SPEC_LEVELS,
        unit_hash::LIGHT_LENGTHS,
        unit_hash::LIGHT_CULL_DISTANCES,
        unit_hash::LIGHT_FALLOFF_STARTS,
        unit_hash::LIGHT_IS_VOLUMETRIC,
        unit_hash::LIGHT_VOL_STRENGTHS,
        unit_hash::EFFECT_IDS,
        unit_hash::EFFECT_TRANSFORMS,
        unit_hash::EFFECT_ALWAYS_ZERO,
        unit_hash::EFFECT_REF_IDS,
        unit_hash::EFFECT_ENABLED,
        unit_hash::SPAWNER_IDS,
        unit_hash::SPAWNER_TRANSFORMS,
        unit_hash::SPAWNER_COUNT,
        unit_hash::SPAWNER_COUNT_RANDOM,
        unit_hash::SPAWNER_AGENT,
        unit_hash::PATH_POINT_POINTS,
        unit_hash::PATH_POINT_TANGENTS,
        unit_hash::PATH_POINT_INDICES,
        unit_hash::PATH_PAIRS,
    ]);
    for bin in 0..unit_hash::PROP_BINS as u32 {
        hashes.extend([
            unit_hash::PROP_ID_BASE + bin,
            unit_hash::PROP_TRANSFORM_BASE + bin,
            unit_hash::PROP_SLOT_BASE + bin,
            unit_hash::PROP_RANDOMIZE_SLOT_BASE + bin,
            unit_hash::PROP_PERCENT_FILL_BASE + bin,
        ]);
    }
    for category in 0..unit_hash::DECAL_CATEGORIES as u32 {
        hashes.extend([
            unit_hash::DECAL_ID_BASE + category,
            unit_hash::DECAL_TRANSFORM_BASE + category,
            unit_hash::DECAL_DEPTH_BASE + category,
            unit_hash::DECAL_MATERIAL_BASE + category,
        ]);
    }
    hashes
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneDocument {
    pub lot_size: Option<[f32; 2]>,
    pub lot_offset: Option<[f32; 2]>,
    pub placement: Option<Vec<f32>>,
    /// Model pseudo-unit transform (`editor.workbench.modelTransform`,
    /// WPF row-major 12 floats) baked into the compiled model vertices.
    pub model_transform: Option<[f32; 12]>,
    pub units: Vec<SceneUnit>,
    pub paths: Vec<ScenePath>,
}

impl SceneDocument {
    /// True when the document carries any unit or path entity the builder
    /// must lower into the Lot property.
    pub fn has_semantics(&self) -> bool {
        !self.units.is_empty() || !self.paths.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenePath {
    pub entry: Option<String>,
    /// Ordered pathPoint unit ids (document order defines the path).
    pub point_ids: Vec<String>,
    /// Workbench scope string; "local" is the engine default (missing = local).
    pub scope_local: bool,
    /// Workbench roadConnect toggle — writes the vanilla road-connect key.
    pub road_connect: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SceneUnit {
    Light {
        transform: Option<Transform>,
        light_type: Option<u32>,
        color: Option<[f32; 3]>,
        outer_radius: Option<f32>,
        inner_radius: Option<f32>,
        diffuse: Option<f32>,
        specular: Option<f32>,
        length: Option<f32>,
        cull_distance: Option<u32>,
        falloff_start: Option<f32>,
        is_volumetric: Option<bool>,
        volumetric_strength: Option<f32>,
        debug_name: Option<String>,
    },
    Effect {
        transform: Option<Transform>,
        effect_id: Option<Key>,
        external_id: Option<Key>,
        enabled: Option<bool>,
    },
    Decal {
        category: usize,
        transform: Option<Transform>,
        scale: Option<f32>,
        depth: Option<f32>,
        material_data: Option<[f32; 3]>,
    },
    Prop {
        bin: usize,
        resource_id: Option<u32>,
        transform: Option<Transform>,
        slot: Option<i32>,
        scale: Option<f32>,
        randomize_slot: Option<bool>,
        percent_fill: Option<bool>,
    },
    PathPoint {
        point: Option<[f32; 3]>,
        tangent: Option<[f32; 3]>,
        point_index: Option<i32>,
        /// Document id (`pathPoint:N`) referenced by path entities.
        id: String,
    },
    Spawner {
        transform: Transform,
        /// 名字 id（UnitLocation 键）。引擎把它当「残缺 TGI」：type/group 恒 0。
        id: Key,
    },
}

/// Parse the subset of the workbench document the builder consumes.  Field
/// names follow the `pe_schema.rs` unit contract.
pub fn parse_document(json: &Json) -> Result<SceneDocument, String> {
    let mut document = SceneDocument::default();
    if let Some(lot) = json.get("lot") {
        document.lot_size = lot
            .get("size")
            .and_then(Json::as_array)
            .filter(|values| values.len() == 2 && values.iter().all(Json::is_number))
            .map(|values| [f32_of(&values[0]), f32_of(&values[1])]);
        document.lot_offset = lot
            .get("overlayBoxOffset")
            .and_then(Json::as_array)
            .filter(|values| values.len() == 2 && values.iter().all(Json::is_number))
            .map(|values| [f32_of(&values[0]), f32_of(&values[1])]);
        document.placement = lot
            .get("placement")
            .and_then(Json::as_array)
            .filter(|values| values.iter().all(Json::is_number))
            .map(|values| values.iter().map(f32_of).collect());
    }
    document.model_transform = json
        .pointer("/editor/workbench/modelTransform")
        .and_then(Json::as_array)
        .filter(|values| values.len() == 12 && values.iter().all(Json::is_number))
        .map(|values| values.iter().map(f32_of).collect::<Vec<f32>>())
        .and_then(|matrix| Some(matrix).filter(|m| m.len() == 12))
        .map(|matrix| {
            let mut out = [0f32; 12];
            out.copy_from_slice(&matrix);
            out
        });
    if let Some(units) = json.get("units").and_then(Json::as_array) {
        for unit in units {
            document.units.push(parse_unit(unit)?);
        }
    }
    validate_spawner_anchors(&document)?;
    if let Some(paths) = json
        .pointer("/editor/workbench/paths")
        .and_then(Json::as_array)
    {
        for path in paths {
            let scope = match path.get("scope") {
                None | Some(Json::Null) => "local",
                Some(Json::String(scope)) => match scope.as_str() {
                    "local" | "global" => scope.as_str(),
                    other => {
                        return Err(format!(
                            "path scope must be \"local\" or \"global\", got {other:?}"
                        ))
                    }
                },
                Some(other) => {
                    return Err(format!("path scope must be a string, got {other:?}"))
                }
            };
            document.paths.push(ScenePath {
                entry: path.get("entry").and_then(Json::as_str).map(str::to_string),
                point_ids: path
                    .get("pointIds")
                    .and_then(Json::as_array)
                    .map(|values| {
                        values
                            .iter()
                            .filter_map(Json::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
                scope_local: scope == "local",
                road_connect: path
                    .get("roadConnect")
                    .and_then(Json::as_bool)
                    .unwrap_or(false),
            });
        }
    }
    Ok(document)
}

/// Import the 0.8.x packager sidecars when a project was exported from the
/// Blender plug-in.  The sidecars deliberately use the same meter/+Z/local
/// contract as the in-editor unit document, so they lower into the exact same
/// native parallel columns and receive the same validation/readback checks.
pub fn merge_sidecar_manifests(
    root: &std::path::Path,
    model: &str,
    document: &mut SceneDocument,
) -> Result<(), String> {
    let model_path = root.join(model);
    let props_path = model_path.with_extension("props.json");
    if props_path.is_file() {
        let value: Json = serde_json::from_slice(
            &std::fs::read(&props_path).map_err(|e| format!("prop manifest: {e}"))?,
        )
        .map_err(|e| format!("prop manifest JSON: {e}"))?;
        let instances = value
            .get("instances")
            .and_then(Json::as_array)
            .ok_or("prop manifest: instances must be an array")?;
        for (index, item) in instances.iter().enumerate() {
            let preset = item.get("preset").and_then(Json::as_str).unwrap_or("");
            let resource_id = item
                .get("resourceId")
                .and_then(parse_json_u32)
                .or_else(|| native_prop_resource_id(preset));
            let matrix = matrix_field(item.get("matrix"))?;
            document.units.push(SceneUnit::Prop {
                bin: native_prop_bin(preset).unwrap_or(0),
                resource_id,
                transform: Some(Transform { flags: 15, unknown: None, matrix: matrix.to_vec() }),
                slot: Some(index as i32),
                scale: f32_field(item.get("scale")),
                randomize_slot: Some(false),
                percent_fill: Some(false),
            });
        }
    }
    let lights_path = model_path.with_extension("lights.json");
    if lights_path.is_file() {
        let value: Json = serde_json::from_slice(
            &std::fs::read(&lights_path).map_err(|e| format!("light manifest: {e}"))?,
        )
        .map_err(|e| format!("light manifest JSON: {e}"))?;
        let instances = value
            .get("instances")
            .and_then(Json::as_array)
            .ok_or("light manifest: instances must be an array")?;
        for item in instances {
            let matrix = matrix_field(item.get("matrix"))?;
            let color = vec3_field(item.get("color"));
            document.units.push(SceneUnit::Light {
                transform: Some(Transform { flags: 14, unknown: None, matrix: matrix.to_vec() }),
                light_type: match item.get("type").and_then(Json::as_str) {
                    Some("SPOT") => Some(unit_hash::LIGHT_TYPE_SPOT),
                    Some("LINE") => Some(unit_hash::LIGHT_TYPE_LINE),
                    _ => Some(unit_hash::LIGHT_TYPE_POINT),
                },
                color,
                outer_radius: f32_field(item.get("radius")),
                inner_radius: f32_field(item.get("innerRadius")),
                diffuse: f32_field(item.get("intensity")),
                specular: f32_field(item.get("specular")),
                length: f32_field(item.get("length")),
                cull_distance: Some(unit_hash::CULL_MAX),
                falloff_start: Some(0.0),
                is_volumetric: Some(false),
                volumetric_strength: Some(0.0),
                debug_name: item.get("id").and_then(Json::as_str).map(|id| format!("SC5_{id}")),
            });
        }
    }
    Ok(())
}

fn matrix_field(value: Option<&Json>) -> Result<[f32; 12], String> {
    let values = value.and_then(Json::as_array).ok_or("manifest matrix missing")?;
    if values.len() != 12 || values.iter().any(|v| !v.is_number()) {
        return Err("manifest matrix must contain 12 numbers".into());
    }
    let mut out = [0.0; 12];
    for (dst, src) in out.iter_mut().zip(values) { *dst = f32_of(src); }
    Ok(out)
}

fn native_prop_resource_id(preset: &str) -> Option<u32> {
    match preset {
        "tree_a" => Some(0x1498_4C68),
        "tree_b" => Some(0x1498_4C69),
        "tree_c" => Some(0x1498_4C6A),
        "tree_conifer" => Some(0x1498_4C6B),
        "street_furniture" => Some(0x9375_C65E),
        // 普通建筑车辆（2026-11-04 普查 crates/sc-properties/examples/
        // car_census.rs：普适车辆原型有且仅有 CarW1/W2/W3 三档，建筑财富
        // 对角线零反例；原版车辆 bin = 0，热气球公园与 R_Gen_W2D2 锚点一致）
        "car_w1" => Some(0x0AB4_DFE1),
        "car_w2" => Some(0x0AB4_DFE2),
        "car_w3" => Some(0x0AB4_DFE3),
        _ => None,
    }
}

fn native_prop_bin(preset: &str) -> Option<usize> {
    match preset {
        "tree_a" => Some(12),
        "tree_b" => Some(11),
        "tree_c" => Some(10),
        "tree_conifer" => Some(9),
        "street_furniture" | "car_w1" | "car_w2" | "car_w3" => Some(0),
        _ => None,
    }
}

/// 车辆等资源驱动 bin 的填充序规范化（engine cUnitBinDraw：可见槽位数 =
/// EcoResourceBin 的 mAmount，按 slot 序逐位填充；PercentFill 置位时 =
/// ceil(Amount/Capacity × 槽位总数)）。原版数据 slot 恒为 bin 内 0..n-1
/// 连号（car_census 实证：公园 17 车位 slot 0..16、R_Gen_W2D2 镇屋 bin0
/// 0..9）。作者显式给出的连号排列保持原样；其余情况（放置克隆继承同一
/// 模板槽号、manifest 全局 enumerate 序号跨 bin 断层）按场景顺序重编号
/// 0..n-1——否则按序填充会出现「槽号 5..7 永不显示」的静默丢车位。
fn canonicalize_prop_slots(slots: &mut [Option<Value>]) {
    let explicit: Vec<Option<i32>> = slots
        .iter()
        .map(|value| match value {
            Some(Value::Int32(slot)) => Some(*slot),
            _ => None,
        })
        .collect();
    let all_set = explicit.iter().all(Option::is_some);
    let is_permutation = explicit
        .iter()
        .filter_map(|slot| *slot)
        .collect::<std::collections::BTreeSet<i32>>()
        == (0..slots.len() as i32).collect::<std::collections::BTreeSet<i32>>();
    if all_set && is_permutation {
        return;
    }
    for (index, slot) in slots.iter_mut().enumerate() {
        *slot = Some(Value::Int32(index as i32));
    }
}

fn parse_unit(unit: &Json) -> Result<SceneUnit, String> {
    let kind = unit
        .get("kind")
        .and_then(Json::as_str)
        .ok_or("unit is missing kind")?;
    let transform = || -> Result<Option<Transform>, String> {
        match unit.get("transform") {
            None | Some(Json::Null) => Ok(None),
            Some(value) => {
                let matrix = value
                    .get("matrix")
                    .and_then(Json::as_array)
                    .ok_or("unit transform is missing matrix")?;
                if matrix.len() != 12 || matrix.iter().any(|v| !v.is_number()) {
                    return Err("unit transform matrix must contain 12 finite numbers".into());
                }
                let flags = value.get("flags").and_then(parse_json_u32).unwrap_or(15);
                Ok(Some(Transform {
                    flags: u16::try_from(flags).unwrap_or(15),
                    unknown: None,
                    matrix: matrix.iter().map(f32_of).collect(),
                }))
            }
        }
    };
    let key_field = |name: &str| -> Option<Key> {
        let field = unit.get(name)?;
        Some(Key {
            type_id: field.get("typeId").and_then(parse_json_u32).unwrap_or(0),
            group: field.get("group").and_then(parse_json_u32).unwrap_or(0),
            instance: field.get("instance").and_then(parse_json_u32)?,
        })
    };
    match kind {
        "light" => Ok(SceneUnit::Light {
            transform: transform()?,
            light_type: unit
                .get("lightType")
                .and_then(Json::as_str)
                .and_then(light_type_instance),
            color: vec3_field(unit.get("color")),
            outer_radius: f32_field(unit.get("outerRadius")),
            inner_radius: f32_field(unit.get("innerRadius")),
            diffuse: f32_field(unit.get("diffuse")),
            specular: f32_field(unit.get("specular")),
            length: f32_field(unit.get("length")),
            cull_distance: unit
                .get("cullDistance")
                .and_then(Json::as_str)
                .and_then(cull_distance_instance),
            falloff_start: f32_field(unit.get("falloffStart")),
            is_volumetric: unit.get("isVolumetric").and_then(Json::as_bool),
            volumetric_strength: f32_field(unit.get("volumetricStrength")),
            debug_name: unit
                .get("debugName")
                .and_then(Json::as_str)
                .map(str::to_string),
        }),
        "effect" => Ok(SceneUnit::Effect {
            transform: transform()?,
            effect_id: key_field("effectId"),
            external_id: key_field("externalId"),
            enabled: unit.get("enabled").and_then(Json::as_bool),
        }),
        "decal" => Ok(SceneUnit::Decal {
            category: unit.get("category").and_then(parse_json_u32).unwrap_or(0) as usize,
            transform: transform()?,
            scale: f32_field(unit.get("scale")),
            depth: f32_field(unit.get("depth")),
            material_data: vec3_field(unit.get("materialData")),
        }),
        "prop" => Ok(SceneUnit::Prop {
            bin: unit.get("bin").and_then(parse_json_u32).unwrap_or(0) as usize,
            resource_id: unit
                .get("resourceId")
                .and_then(parse_json_u32)
                // Direct-model placements carry the RW4 TGI instead.
                .or_else(|| unit.pointer("/modelTgi/instance").and_then(parse_json_u32)),
            transform: transform()?,
            slot: unit.get("slot").and_then(parse_json_u32).map(|v| v as i32),
            scale: f32_field(unit.get("scale")),
            randomize_slot: unit.get("randomizeSlot").and_then(Json::as_bool),
            percent_fill: unit.get("percentFill").and_then(Json::as_bool),
        }),
        "pathPoint" => Ok(SceneUnit::PathPoint {
            point: vec3_field(unit.get("point")),
            tangent: vec3_field(unit.get("tangent")),
            point_index: unit
                .get("pointIndex")
                .and_then(parse_json_u32)
                .map(|v| v as i32),
            id: unit
                .get("id")
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_string(),
        }),
        "spawner" => {
            // 生成器锚点写入契约（docs/re/spawner-render-and-assembly-20261101.md）：
            // ① 名字 id 必填且非零——引擎按名字建 mUnitLocations，缺名即无法取点；
            // ② 名字 key 的 type/group 恒 0（残缺 TGI 约定）；
            // ③ 局部变换必填——ids / transforms 是并行数组，缺一行会让整列错位。
            let label = unit
                .get("id")
                .and_then(Json::as_str)
                .unwrap_or("spawner");
            let id = key_field("spawnerId")
                .map(|key| Key {
                    type_id: 0,
                    group: 0,
                    instance: key.instance,
                })
                .filter(|key| key.instance != 0)
                .ok_or_else(|| {
                    format!("{label}: spawnerId 缺失或为 0（锚点必须带名字 id）")
                })?;
            let transform = transform()?.ok_or_else(|| {
                format!("{label}: 缺少 transform（锚点写入需要局部变换）")
            })?;
            Ok(SceneUnit::Spawner { transform, id })
        }
        other => Err(format!("unsupported unit kind {other}")),
    }
}

fn light_type_instance(name: &str) -> Option<u32> {
    match name {
        "Point" => Some(unit_hash::LIGHT_TYPE_POINT),
        "Spot" => Some(unit_hash::LIGHT_TYPE_SPOT),
        "Line" => Some(unit_hash::LIGHT_TYPE_LINE),
        _ => None,
    }
}

fn cull_distance_instance(name: &str) -> Option<u32> {
    match name {
        "Near" => Some(unit_hash::CULL_NEAR),
        "Mid" => Some(unit_hash::CULL_MID),
        "Far" => Some(unit_hash::CULL_FAR),
        "Max" => Some(unit_hash::CULL_MAX),
        _ => None,
    }
}

fn f32_of(value: &Json) -> f32 {
    value.as_f64().map(|v| v as f32).unwrap_or_default()
}

/// Parse a `0x…`/decimal string (or JSON number) as u32.
pub fn parse_json_u32(value: &Json) -> Option<u32> {
    value
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .or_else(|| {
            value
                .as_str()
                .and_then(|value| value.parse().ok().or_else(|| parse_hex_u32(value)))
        })
}

/// Parse `{"typeId":…,"group":…,"instance":…}` (numbers or hex strings).
pub fn parse_json_tgi(value: &Json) -> Option<ResourceId> {
    Some(ResourceId {
        type_id: parse_json_u32(value.get("typeId")?)?,
        group: parse_json_u32(value.get("group")?)?,
        instance: parse_json_u32(value.get("instance")?)?,
    })
}

/// Parse a `1a2b3c4d` / `0x…` hex string as u32.
pub fn parse_hex_u32(value: &str) -> Option<u32> {
    u32::from_str_radix(
        value
            .trim()
            .trim_start_matches("0x")
            .trim_start_matches("0X"),
        16,
    )
    .ok()
}

fn f32_field(value: Option<&Json>) -> Option<f32> {
    value.and_then(Json::as_f64).map(|v| v as f32)
}

fn vec3_field(value: Option<&Json>) -> Option<[f32; 3]> {
    let values = value?.as_array()?;
    Some([
        f32_of(values.first()?),
        f32_of(values.get(1)?),
        f32_of(values.get(2)?),
    ])
}

fn key_of(instance: u32) -> Value {
    Value::Key(Key {
        type_id: 0,
        group: 0,
        instance,
    })
}

fn key_value(key: Key) -> Value {
    Value::Key(key)
}

fn transform_value(scale: Option<f32>, transform: &Option<Transform>) -> Option<Value> {
    let mut transform = transform.clone()?;
    // The encoder requires (flags == 15) ⇔ unknown.is_some(); flags 15 carries
    // the prop/decal Scale in `unknown`. Scale-less transforms (lights,
    // spawners, path-side props) use flags 14 — the vanilla convention
    // (E917279C: every transform is 14 or 15, never 0; flags 0 poisons the
    // property parse and the whole lot/model disappears).
    transform.unknown = scale;
    transform.flags = if transform.unknown.is_some() { 15 } else { 14 };
    Some(Value::Transform(transform))
}

/// Replace the property's unit columns with the scene document's units.
/// Every original unit column is removed first — this is what keeps a
/// referenced lot's own props/lights/paths from resurfacing in the build.
pub fn apply_units(file: &mut PropertyFile, document: &SceneDocument) -> Result<(), String> {
    let mut stripped = unit_column_hashes();
    // The path binding trio (entry/scope/road-connect) is rewritten whenever
    // the document defines paths; strip the referenced lot's originals in that
    // case or the writeback duplicates the hash (s14's [03106C2E] vs authored).
    // A document without paths keeps the referenced columns untouched.
    if !document.paths.is_empty() {
        stripped.extend([
            0x0CC8_FE7A, // path entry keys
            0x0D9A_4E54, // scope
            0x0D9A_A2BB, // road connect
        ]);
    }
    let mut columns: Vec<Property> = Vec::new();
    let mut lights: ColumnSet = ColumnSet::default();
    let mut effects: ColumnSet = ColumnSet::default();
    let mut spawners: ColumnSet = ColumnSet::default();
    let mut path_points: ColumnSet = ColumnSet::default();
    let mut path_point_ids: Vec<String> = Vec::new();
    let mut path_point_indices: Vec<Option<i32>> = Vec::new();
    let mut prop_bins: std::collections::BTreeMap<usize, ColumnSet> = Default::default();
    let mut decals: std::collections::BTreeMap<usize, ColumnSet> = Default::default();
    // Light group / night period come from native_profile's author-aligned
    // constants (single source of truth; the decimal-pinned copy is tested
    // in native_profile::tests::light_profile_constants_match_author_json).
    const LIGHT_GROUP: u32 = super::native_profile::LIGHT_GROUP;

    for unit in &document.units {
        match unit {
            SceneUnit::Light {
                transform,
                light_type,
                color,
                outer_radius,
                inner_radius,
                diffuse,
                specular,
                length,
                cull_distance,
                falloff_start,
                is_volumetric,
                debug_name,
                volumetric_strength,
            } => lights.push([
                ("ids", Some(key_of(LIGHT_GROUP))),
                ("types", light_type.map(key_of)),
                ("transforms", transform_value(None, transform)),
                ("colors", color.map(|[r, g, b]| Value::ColorRgb { r, g, b })),
                ("radii", outer_radius.map(Value::Float)),
                ("inner_radii", inner_radius.map(Value::Float)),
                ("diffuse", diffuse.map(Value::Float)),
                ("specular", specular.map(Value::Float)),
                ("lengths", length.map(Value::Float)),
                ("cull", cull_distance.map(key_of)),
                ("falloff", falloff_start.map(Value::Float)),
                ("volumetric", is_volumetric.map(Value::Bool)),
                ("vol_strength", volumetric_strength.map(Value::Float)),
                ("names", debug_name.clone().map(Value::String8)),
            ]),
            SceneUnit::Effect {
                transform,
                effect_id,
                external_id,
                enabled,
            } => effects.push([
                ("ids", effect_id.map(key_value)),
                ("transforms", transform_value(None, transform)),
                // effectWorlds(0x02A907B9):原版恒零键(消防局/水泵/理工
                // 实证);EFFECT_ALWAYS_ZERO 是历史误名。
                ("worlds", Some(Value::Key(Key { type_id: 0, group: 0, instance: 0 }))),
                ("refs", external_id.map(key_value)),
                ("enabled", enabled.map(Value::Bool)),
            ]),
            SceneUnit::Decal {
                category,
                transform,
                scale,
                depth,
                material_data,
            } => decals.entry(*category).or_default().push([
                ("transforms", transform_value(*scale, transform)),
                ("depths", depth.map(Value::Float)),
                ("materials", material_data.map(Value::Vector3)),
            ]),
            SceneUnit::Prop {
                bin,
                resource_id,
                transform,
                slot,
                scale,
                randomize_slot,
                percent_fill,
            } => prop_bins.entry(*bin).or_default().push([
                ("ids", resource_id.map(key_of)),
                ("transforms", transform_value(*scale, transform)),
                ("slots", slot.map(Value::Int32)),
                ("randomize", randomize_slot.map(Value::Bool)),
                ("percent_fill", percent_fill.map(Value::Bool)),
            ]),
            SceneUnit::PathPoint {
                point,
                tangent,
                point_index,
                id,
            } => {
                path_points.push([
                    ("points", point.map(Value::Vector3)),
                    ("tangents", tangent.map(Value::Vector3)),
                    ("indices", point_index.map(Value::Int32)),
                ]);
                path_point_ids.push(id.clone());
                path_point_indices.push(*point_index);
            }
            SceneUnit::Spawner { transform, id } => spawners.push([
                ("ids", Some(key_value(*id))),
                ("transforms", transform_value(None, &Some(transform.clone()))),
            ]),
        }
    }

    lights.finish(
        &mut columns,
        &[
            (unit_hash::LIGHT_IDS, "ids", PropType::Key),
            (unit_hash::LIGHT_TYPES, "types", PropType::Key),
            (
                unit_hash::LIGHT_TRANSFORMS,
                "transforms",
                PropType::Transform,
            ),
            (unit_hash::LIGHT_COLORS, "colors", PropType::ColorRgb),
            (unit_hash::LIGHT_RADII, "radii", PropType::Float),
            (unit_hash::LIGHT_INNER_RADII, "inner_radii", PropType::Float),
            (unit_hash::LIGHT_DIFFUSE_LEVELS, "diffuse", PropType::Float),
            (unit_hash::LIGHT_SPEC_LEVELS, "specular", PropType::Float),
            (unit_hash::LIGHT_LENGTHS, "lengths", PropType::Float),
            (unit_hash::LIGHT_CULL_DISTANCES, "cull", PropType::Key),
            (unit_hash::LIGHT_FALLOFF_STARTS, "falloff", PropType::Float),
            (unit_hash::LIGHT_IS_VOLUMETRIC, "volumetric", PropType::Bool),
            (unit_hash::LIGHT_VOL_STRENGTHS, "vol_strength", PropType::Float),
            (unit_hash::LIGHT_DEBUG_NAMES, "names", PropType::String8),
        ],
    );
    if document.units.iter().any(|u| matches!(u, SceneUnit::Light { .. })) {
        columns.push(array_property(
            0x0CE7_1B10,
            PropType::Key,
            vec![key_of(LIGHT_GROUP)],
        ));
        columns.push(array_property(
            0x0CE7_1B11,
            PropType::Key,
            // Night period instance — author light_native_profile.json
            // nightPeriod 311764537 == 0x1295_2639 (golden package byte-
            // verified). 0x1295_6239 was a transcription error that kept
            // University v9-v11 lights dark.
            vec![key_of(super::native_profile::NIGHT_PERIOD)],
        ));
        columns.push(array_property(
            0x0CE7_1B12,
            PropType::Vector2,
            vec![Value::Vector2([0.875, 0.21527299])],
        ));
        columns.push(array_property(
            0x0CE7_1B13,
            PropType::Vector2,
            vec![Value::Vector2([0.0, 0.0])],
        ));
    }
    effects.finish(
        &mut columns,
        &[
            (unit_hash::EFFECT_IDS, "ids", PropType::Key),
            (
                unit_hash::EFFECT_TRANSFORMS,
                "transforms",
                PropType::Transform,
            ),
            (unit_hash::EFFECT_WORLDS, "worlds", PropType::Key),
            (unit_hash::EFFECT_REF_IDS, "refs", PropType::Key),
            (unit_hash::EFFECT_ENABLED, "enabled", PropType::Bool),
        ],
    );
    spawners.finish(
        &mut columns,
        &[
            (unit_hash::SPAWNER_IDS, "ids", PropType::Key),
            (
                unit_hash::SPAWNER_TRANSFORMS,
                "transforms",
                PropType::Transform,
            ),
        ],
    );
    path_points.finish(
        &mut columns,
        &[
            (unit_hash::PATH_POINT_POINTS, "points", PropType::Vector3),
            (
                unit_hash::PATH_POINT_TANGENTS,
                "tangents",
                PropType::Vector3,
            ),
            (unit_hash::PATH_POINT_INDICES, "indices", PropType::Int32),
        ],
    );
    for (bin, column) in prop_bins {
        if bin >= unit_hash::PROP_BINS {
            return Err(format!("prop bin {bin} is outside the 14 supported bins"));
        }
        let bin = bin as u32;
        let mut column = column;
        if let Some(slots) = column.fields.get_mut("slots") {
            canonicalize_prop_slots(slots);
        }
        column.finish(
            &mut columns,
            &[
                (unit_hash::PROP_ID_BASE + bin, "ids", PropType::Key),
                (
                    unit_hash::PROP_TRANSFORM_BASE + bin,
                    "transforms",
                    PropType::Transform,
                ),
                (unit_hash::PROP_SLOT_BASE + bin, "slots", PropType::Int32),
                (
                    unit_hash::PROP_RANDOMIZE_SLOT_BASE + bin,
                    "randomize",
                    PropType::Bool,
                ),
                (
                    unit_hash::PROP_PERCENT_FILL_BASE + bin,
                    "percent_fill",
                    PropType::Bool,
                ),
            ],
        );
    }
    for (category, column) in decals {
        if category >= unit_hash::DECAL_CATEGORIES {
            continue;
        }
        let category = category as u32;
        column.finish(
            &mut columns,
            &[
                (
                    unit_hash::DECAL_TRANSFORM_BASE + category,
                    "transforms",
                    PropType::Transform,
                ),
                (
                    unit_hash::DECAL_DEPTH_BASE + category,
                    "depths",
                    PropType::Float,
                ),
                (
                    unit_hash::DECAL_MATERIAL_BASE + category,
                    "materials",
                    PropType::Vector3,
                ),
            ],
        );
    }
    finish_path_pairs(document, &path_point_ids, &path_point_indices, &mut columns);

    // Path-entry bindings (0CC8FE7A): full TGI keys linking the lot to its
    // script-side PathEntry resources — vanilla asset dicts carry exactly
    // this column (E917279C binds three, e.g. [instance, 00B1B104,
    // 40E0C400]); without it the path never links. The university door
    // entry 675B61B1 is the known 40E1C400 resource exception.
    let entries: Vec<Value> = document
        .paths
        .iter()
        .filter_map(|path| {
            path.entry.as_deref().and_then(|entry| {
                u32::from_str_radix(entry.trim_start_matches("0x"), 16)
                    .ok()
                    .map(|instance| {
                        Value::Key(sc_properties::Key {
                            type_id: 0x00B1_B104,
                            group: super::native_profile::path_entry_group(instance),
                            instance,
                        })
                    })
            })
        })
        .collect();
    if !entries.is_empty() {
        columns.push(array_property(
            0x0CC8_FE7A,
            PropType::Key,
            entries,
        ));
    }
    // Scope + road-connect keys, one per path — per-path engine enums
    // (SC::tPathScope / SC::tRoadConnect, PDB 值见 native_profile 常量)：
    // 原版小学三组 = [Global+First, Local+None, Local+None]；connect 列恒写。
    if !document.paths.is_empty() {
        let mut scopes = Vec::with_capacity(document.paths.len());
        let mut connects = Vec::with_capacity(document.paths.len());
        for path in &document.paths {
            scopes.push(Value::Key(Key {
                type_id: 0,
                group: 0,
                instance: if path.scope_local {
                    super::native_profile::PATH_SCOPE_LOCAL
                } else {
                    super::native_profile::PATH_SCOPE_GLOBAL
                },
            }));
            connects.push(Value::Key(Key {
                type_id: 0,
                group: 0,
                instance: if path.road_connect {
                    super::native_profile::PATH_ROAD_CONNECT_FIRST
                } else {
                    super::native_profile::PATH_ROAD_CONNECT_NONE
                },
            }));
        }
        columns.push(array_property(0x0D9A_4E54, PropType::Key, scopes));
        columns.push(array_property(0x0D9A_A2BB, PropType::Key, connects));
    }

    // The writer's own output must replace any referenced originals: strip
    // every column hash we are about to write (or already stripped above)
    // before extending, otherwise a referenced lot's surviving column with
    // the same hash turns the writeback into a duplicate-hash property file.
    for column in &columns {
        stripped.push(column.hash);
    }
    stripped.sort_unstable();
    stripped.dedup();
    file.values
        .retain(|property| !stripped.contains(&property.hash));
    file.values.extend(columns);
    file.claimed_count = file.values.len() as u32;
    Ok(())
}

/// PATH_PAIRS: per path entity the (first, last) global point indices — the
/// C# `UnitFile.UnitPath` pair convention.
fn finish_path_pairs(
    document: &SceneDocument,
    point_ids: &[String],
    point_indices: &[Option<i32>],
    columns: &mut Vec<Property>,
) {
    if document.paths.is_empty() {
        return;
    }
    let index_of_id: std::collections::HashMap<&str, i32> = point_ids
        .iter()
        .zip(point_indices.iter())
        .filter_map(|(id, index)| index.map(|i| (id.as_str(), i)))
        .collect();
    let mut pairs: Vec<Value> = Vec::new();
    for path in &document.paths {
        let mut indices: Vec<i32> = path
            .point_ids
            .iter()
            .filter_map(|id| index_of_id.get(id.as_str()).copied())
            .collect();
        if indices.is_empty() {
            continue;
        }
        indices.sort_unstable();
        pairs.push(Value::Int32(indices[0]));
        pairs.push(Value::Int32(indices[indices.len() - 1]));
    }
    if !pairs.is_empty() {
        columns.push(array_property(
            unit_hash::PATH_PAIRS,
            PropType::Int32,
            pairs,
        ));
    }
}

fn array_property(hash: u32, prop_type: PropType, values: Vec<Value>) -> Property {
    // Fixed-size columns must declare their stride (the canonical encoder
    // validates it); Transform/String8 are variable-size and declare none.
    let array_item_size = match prop_type {
        PropType::Bool => Some(1),
        PropType::Int32 | PropType::UInt32 | PropType::Float => Some(4),
        PropType::Key | PropType::Vector3 | PropType::ColorRgb => Some(12),
        PropType::Vector2 => Some(8),
        _ => None,
    };
    Property {
        hash,
        prop_type,
        kind: Kind::Array(values),
        encoding: PropertyEncoding {
            flags: 0,
            array_item_size,
        },
    }
}

/// Per-kind column builder: each `push` contributes one unit's optional field
/// values; `finish` truncates every column at its first absent field (the
/// native parallel-array convention) and emits non-empty columns.
#[derive(Default)]
struct ColumnSet {
    fields: std::collections::BTreeMap<&'static str, Vec<Option<Value>>>,
}

impl ColumnSet {
    fn push<const N: usize>(&mut self, values: [(&'static str, Option<Value>); N]) {
        for (name, value) in values {
            self.fields.entry(name).or_default().push(value);
        }
    }

    fn finish(&self, columns: &mut Vec<Property>, spec: &[(u32, &'static str, PropType)]) {
        for (hash, name, prop_type) in spec {
            let Some(values) = self.fields.get(name) else {
                continue;
            };
            // Truncate at the first absent field (short-column convention).
            let end = values
                .iter()
                .position(Option::is_none)
                .unwrap_or(values.len());
            if end == 0 {
                continue;
            }
            columns.push(array_property(
                *hash,
                *prop_type,
                values[..end].iter().flatten().cloned().collect(),
            ));
        }
    }
}

/// 生成器锚点的文档级校验：名字 id 在同一 lot 内必须唯一。
///
/// 引擎把 `0x0E1BAC61` 装配成 `vector_map<名字 id, 变换>`，同名锚点后写覆盖先写——
/// 两个重叠的锚点会静默吞掉一个，是"编辑器里看得见、游戏里少一个"的经典来源。
fn validate_spawner_anchors(document: &SceneDocument) -> Result<(), String> {
    let mut seen: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for unit in &document.units {
        let SceneUnit::Spawner { id, .. } = unit else {
            continue;
        };
        *seen.entry(id.instance).or_default() += 1;
    }
    let duplicates: Vec<String> = seen
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(instance, count)| format!("0x{instance:08X} ×{count}"))
        .collect();
    if duplicates.is_empty() {
        return Ok(());
    }
    Err(format!(
        "生成器锚点名字 id 重复（引擎按名字建表，同名会互相覆盖）：{}",
        duplicates.join(", ")
    ))
}

/// Verify the lowered property reassembles to the scene's unit counts.
pub fn verify_units(file: &PropertyFile, document: &SceneDocument) -> Result<(), String> {
    let expected_props = document
        .units
        .iter()
        .filter(|unit| matches!(unit, SceneUnit::Prop { .. }))
        .count();
    let expected_points = document
        .units
        .iter()
        .filter(|unit| matches!(unit, SceneUnit::PathPoint { .. }))
        .count();
    let expected_spawners = document
        .units
        .iter()
        .filter(|unit| matches!(unit, SceneUnit::Spawner { .. }))
        .count();
    let assembled = LotEditorDocument::from_property_file(file.clone()).assemble_units();
    let props = assembled
        .units
        .iter()
        .filter(|unit| matches!(unit, sc_properties::LotUnit::Prop { .. }))
        .count();
    let points = assembled
        .units
        .iter()
        .filter(|unit| matches!(unit, sc_properties::LotUnit::PathPoint { .. }))
        .count();
    let spawners = assembled
        .units
        .iter()
        .filter(|unit| matches!(unit, sc_properties::LotUnit::Spawner { .. }))
        .count();
    if props != expected_props || points != expected_points || spawners != expected_spawners {
        return Err(format!(
            "unit writeback readback mismatch: props {props} != {expected_props}, path points {points} != {expected_points}, spawners {spawners} != {expected_spawners}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn property_file() -> PropertyFile {
        PropertyFile {
            claimed_count: 0,
            values: Vec::new(),
        }
    }

    #[test]
    fn parses_units_paths_and_lot_fields() {
        let document = parse_document(&json!({
            "lot": {"size": [80, 80], "overlayBoxOffset": [1, 2]},
            "units": [
                {"id": "light:0", "kind": "light", "index": 0,
                 "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 4,0,2], "flags": 15},
                 "lightType": "Spot", "color": [1, 0.5, 0.25],
                 "outerRadius": 8, "cullDistance": "Far"},
                {"id": "prop:1:0", "kind": "prop", "index": 0, "bin": 1,
                 "resourceId": 3271283064u64, "slot": 0,
                 "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 2,0,4], "flags": 15}},
                {"id": "pathPoint:0", "kind": "pathPoint", "index": 0,
                 "point": [0, 0, 0], "tangent": [1, 0, 0], "pointIndex": 0},
                {"id": "pathPoint:1", "kind": "pathPoint", "index": 1,
                 "point": [8, 0, 0], "tangent": [1, 0, 0], "pointIndex": 1}
            ],
            "editor": {"workbench": {"paths": [
                {"id": "path:1", "entry": "418a18b1",
                 "pointIds": ["pathPoint:0", "pathPoint:1"]}
            ]}}
        }))
        .unwrap();
        assert_eq!(document.lot_size, Some([80.0, 80.0]));
        assert_eq!(document.units.len(), 4);
        assert_eq!(document.paths.len(), 1);
        assert_eq!(document.paths[0].point_ids.len(), 2);
        let SceneUnit::Prop {
            bin, resource_id, ..
        } = &document.units[1]
        else {
            panic!("expected prop");
        };
        assert_eq!((*bin, *resource_id), (1, Some(3271283064)));
    }

    #[test]
    fn applies_units_and_strips_reference_columns() {
        let mut file = property_file();
        // Referenced lot's original units — must not survive the writeback.
        // (Spot = 0x2F0FF9FD; the authored light below is a Point.)
        file.values.push(Property {
            hash: unit_hash::LIGHT_TYPES,
            prop_type: PropType::Key,
            kind: Kind::Array(vec![key_of(0x2F0F_F9FD)]),
            encoding: PropertyEncoding::default(),
        });
        file.values.push(Property {
            hash: unit_hash::PROP_ID_BASE,
            prop_type: PropType::Key,
            kind: Kind::Array(vec![key_of(42)]),
            encoding: PropertyEncoding::default(),
        });
        file.values.push(Property {
            // Non-unit column (LotSize): preserved.
            hash: 0x0CCB_7FC8,
            prop_type: PropType::Vector2,
            kind: Kind::Scalar(Value::Vector2([80.0, 80.0])),
            encoding: PropertyEncoding::default(),
        });
        let document = parse_document(&json!({
            "units": [
                {"id": "prop:13:0", "kind": "prop", "index": 0, "bin": 13,
                 "resourceId": 3271283064u64, "slot": 2,
                 "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 2,0,4], "flags": 15}},
                {"id": "pathPoint:0", "kind": "pathPoint", "index": 0,
                 "point": [0, 0, 0], "pointIndex": 0},
                {"id": "pathPoint:1", "kind": "pathPoint", "index": 1,
                 "point": [8, 0, 0], "pointIndex": 1},
                {"id": "light:0", "kind": "light", "index": 0,
                 "lightType": "Point", "outerRadius": 6}
            ],
            "editor": {"workbench": {"paths": [
                {"id": "path:1", "pointIds": ["pathPoint:0", "pathPoint:1"]}
            ]}}
        }))
        .unwrap();
        apply_units(&mut file, &document).unwrap();
        verify_units(&file, &document).unwrap();

        let hashes: std::collections::HashSet<u32> = file.values.iter().map(|p| p.hash).collect();
        assert!(hashes.contains(&0x0CCB_7FC8), "non-unit column preserved");
        assert!(
            !hashes.contains(&unit_hash::PROP_ID_BASE),
            "bin-0 reference props stripped"
        );
        let lights = file.get(unit_hash::LIGHT_TYPES).unwrap();
        assert_eq!(
            lights.array().unwrap(),
            &[Value::Key(Key {
                type_id: 0,
                group: 0,
                instance: unit_hash::LIGHT_TYPE_POINT
            })],
            "reference Spot light replaced by the authored Point light"
        );
        // New columns present with the right shapes.
        let prop_ids = file.get(unit_hash::PROP_ID_BASE + 13).unwrap();
        assert_eq!(
            prop_ids.array().unwrap()[0],
            Value::Key(Key {
                type_id: 0,
                group: 0,
                instance: 3271283064
            })
        );
        let points = file.get(unit_hash::PATH_POINT_POINTS).unwrap();
        assert_eq!(points.array().unwrap().len(), 2);
        let pairs = file.get(unit_hash::PATH_PAIRS).unwrap();
        assert_eq!(pairs.array().unwrap(), &[Value::Int32(0), Value::Int32(1)]);
        // Round-trip through the canonical encoder.
        let encoded = file.encode_canonical().unwrap();
        let parsed = PropertyFile::parse(&encoded).unwrap();
        verify_units(&parsed, &document).unwrap();
        let reassembled = LotEditorDocument::from_property_file(parsed).assemble_units();
        assert_eq!(
            reassembled.units.len(),
            4,
            "props + pathPoints + light reassemble"
        );
    }

    #[test]
    fn empty_document_clears_all_unit_columns() {
        let mut file = property_file();
        file.values.push(Property {
            hash: unit_hash::LIGHT_TYPES,
            prop_type: PropType::Key,
            kind: Kind::Array(vec![key_of(1)]),
            encoding: PropertyEncoding::default(),
        });
        let document = SceneDocument::default();
        apply_units(&mut file, &document).unwrap();
        assert_eq!(file.values.len(), 0, "empty scene = no unit columns at all");
    }

    #[test]
    fn out_of_range_prop_bin_is_rejected() {
        let mut file = property_file();
        let document = parse_document(&json!({
            "units": [{"id": "prop:99:0", "kind": "prop", "index": 0, "bin": 99}]
        }))
        .unwrap();
        assert!(apply_units(&mut file, &document).is_err());
    }

    // ---- 车辆 bin slot 填充序规范化（2026-11-04 car_census 契约）----

    fn slots_of(file: &PropertyFile) -> Vec<i32> {
        file.values
            .iter()
            .find(|p| p.hash == unit_hash::PROP_SLOT_BASE)
            .map(|p| match &p.kind {
                Kind::Array(vs) => vs
                    .iter()
                    .map(|v| match v {
                        Value::Int32(i) => *i,
                        _ => panic!("slot must be Int32"),
                    })
                    .collect(),
                Kind::Scalar(v) => match v {
                    Value::Int32(i) => vec![*i],
                    _ => panic!("slot must be Int32"),
                },
                Kind::Empty => vec![],
            })
            .unwrap_or_else(|| panic!("slots column must exist"))
    }

    /// 放置克隆继承同一模板槽号（如三辆车都带模板 slot=3）→ 重编号 0..n-1。
    #[test]
    fn duplicated_template_slots_are_renumbered() {
        let mut file = property_file();
        let document = parse_document(&json!({
            "units": [
                {"id": "prop:0", "kind": "prop", "index": 0, "bin": 0,
                 "resourceId": {"typeId": 0, "group": 0, "instance": 1},
                 "slot": 3,
                 "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 0,0,0], "flags": 15}},
                {"id": "prop:1", "kind": "prop", "index": 1, "bin": 0,
                 "resourceId": {"typeId": 0, "group": 0, "instance": 1},
                 "slot": 3,
                 "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 1,0,0], "flags": 15}},
                {"id": "prop:2", "kind": "prop", "index": 2, "bin": 0,
                 "resourceId": {"typeId": 0, "group": 0, "instance": 1},
                 "slot": 3,
                 "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 2,0,0], "flags": 15}}
            ]
        }))
        .unwrap();
        apply_units(&mut file, &document).unwrap();
        assert_eq!(slots_of(&file), [0, 1, 2], "重复模板槽号 → 填充序连号");
    }

    /// 作者显式给出的连号排列（填充顺序有意调换）保持原样。
    #[test]
    fn explicit_slot_permutation_is_preserved() {
        let mut file = property_file();
        let document = parse_document(&json!({
            "units": [
                {"id": "prop:0", "kind": "prop", "index": 0, "bin": 0,
                 "resourceId": {"typeId": 0, "group": 0, "instance": 1},
                 "slot": 1,
                 "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 0,0,0], "flags": 15}},
                {"id": "prop:1", "kind": "prop", "index": 1, "bin": 0,
                 "resourceId": {"typeId": 0, "group": 0, "instance": 1},
                 "slot": 0,
                 "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 1,0,0], "flags": 15}}
            ]
        }))
        .unwrap();
        apply_units(&mut file, &document).unwrap();
        assert_eq!(slots_of(&file), [1, 0], "显式连号排列 = 作者意图，不改");
    }

    /// 缺席 slot（旧场景无槽位列）→ 补齐为 0..n-1（原版形态）。
    #[test]
    fn missing_slots_are_filled_sequentially() {
        let mut file = property_file();
        let document = parse_document(&json!({
            "units": [
                {"id": "prop:0", "kind": "prop", "index": 0, "bin": 0,
                 "resourceId": {"typeId": 0, "group": 0, "instance": 1},
                 "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 0,0,0], "flags": 15}},
                {"id": "prop:1", "kind": "prop", "index": 1, "bin": 0,
                 "resourceId": {"typeId": 0, "group": 0, "instance": 1},
                 "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 1,0,0], "flags": 15}}
            ]
        }))
        .unwrap();
        apply_units(&mut file, &document).unwrap();
        assert_eq!(slots_of(&file), [0, 1], "缺席 slot → 补齐填充序");
    }

    // ---- 生成器锚点写入契约（2026-11-01）----

    fn spawner_unit(instance: u64, x: f64) -> Json {
        json!({
            "id": format!("spawner:{instance}"),
            "kind": "spawner",
            "index": instance,
            "spawnerId": {"typeId": 0, "group": 0, "instance": instance},
            "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, x,0,0], "flags": 15}
        })
    }

    #[test]
    fn spawner_requires_name_id() {
        let err = parse_document(&json!({
            "units": [{
                "id": "spawner:0", "kind": "spawner", "index": 0,
                "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 0,0,0], "flags": 15}
            }]
        }))
        .unwrap_err();
        assert!(err.contains("spawnerId"), "unexpected error: {err}");

        let zero = parse_document(&json!({
            "units": [{
                "id": "spawner:0", "kind": "spawner", "index": 0,
                "spawnerId": {"typeId": 0, "group": 0, "instance": 0},
                "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 0,0,0], "flags": 15}
            }]
        }));
        assert!(zero.is_err(), "instance 0 must be rejected");
    }

    #[test]
    fn spawner_requires_transform() {
        let err = parse_document(&json!({
            "units": [{
                "id": "spawner:1", "kind": "spawner", "index": 1,
                "spawnerId": {"typeId": 0, "group": 0, "instance": 28700},
                "transform": null
            }]
        }))
        .unwrap_err();
        assert!(err.contains("transform"), "unexpected error: {err}");
    }

    #[test]
    fn spawner_rejects_duplicate_names() {
        let err = parse_document(&json!({
            "units": [spawner_unit(0x141EED53, 1.0), spawner_unit(0x141EED53, 2.0)]
        }))
        .unwrap_err();
        assert!(err.contains("141EED53"), "unexpected error: {err}");
    }

    #[test]
    fn spawner_writes_only_ids_and_transforms() {
        let mut file = property_file();
        let document =
            parse_document(&json!({"units": [spawner_unit(0x141EED53, 1.5), spawner_unit(0xABCDEF, 3.0)]}))
                .unwrap();
        apply_units(&mut file, &document).unwrap();
        verify_units(&file, &document).unwrap();

        // 名字 key 规范化：type/group 恒 0。
        let ids = file.get(unit_hash::SPAWNER_IDS).unwrap();
        assert_eq!(
            ids.array().unwrap(),
            &[
                Value::Key(Key { type_id: 0, group: 0, instance: 0x141E_ED53 }),
                Value::Key(Key { type_id: 0, group: 0, instance: 0xABCD_EF }),
            ]
        );
        let transforms = file.get(unit_hash::SPAWNER_TRANSFORMS).unwrap();
        assert_eq!(transforms.array().unwrap().len(), 2, "并行数组等长");
        // 历史误标列不再写入（等长契约 + 语义属建筑变体系统）。
        assert!(file.get(unit_hash::SPAWNER_COUNT).is_none());
        assert!(file.get(unit_hash::SPAWNER_COUNT_RANDOM).is_none());
        assert!(file.get(unit_hash::SPAWNER_AGENT).is_none());

        // canonical 编码往返后仍然对齐。
        let encoded = file.encode_canonical().unwrap();
        let parsed = PropertyFile::parse(&encoded).unwrap();
        verify_units(&parsed, &document).unwrap();
    }
}
