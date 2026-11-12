//! 工作台逻辑编排图 → M1 schema semantics 的编译（2026-11-07）。
//!
//! 输入：工作台文档 `asset.lot.json` 的 `editor.workbench.menu` + `logic`。
//! 输出：`semantics.menu` / `semantics.labor` / `semantics.behaviors` 覆盖值。
//!
//! 这是"画布 → 构建输入"的唯一桥：没有它，菜单绑定与行为节点只是文档注释
//! （r10 之前 `deriveSemanticsMenu` 从未被构建读取）。校验 fail-closed：
//! 图结构、菜单必填项、图标/背景图项目内存在、节点参数与行为冲突都在入队前
//! 报出并点名节点 id。
//!
//! 纯函数（文件存在性由调用方以闭包注入），可单测。

use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::path::Path;

/// 编译结果（只含要覆盖的 semantics 段；None = 保持基础 schema 原值）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CompiledLogic {
    pub menu: Option<Value>,
    pub labor: Option<Value>,
    pub behaviors: Vec<Value>,
    /// 文档是否声明了逻辑图（false = 不触碰 base 的 labor/behaviors）。
    pub logic_present: bool,
    pub warnings: Vec<String>,
}

/// 编译工作台文档（`path_exists` 用于菜单图片的项目内存在性检查）。
pub fn compile(
    document: &Value,
    path_exists: &dyn Fn(&str) -> bool,
) -> Result<CompiledLogic, Vec<String>> {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let workbench = document.pointer("/editor/workbench");

    let menu = compile_menu(workbench, path_exists, &mut errors);
    let (labor, behaviors, logic_present) = compile_logic(workbench, &mut errors, &mut warnings);

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(CompiledLogic {
        menu,
        labor,
        behaviors,
        logic_present,
        warnings,
    })
}

fn hex_ok(value: &str) -> bool {
    let text = value.trim().trim_start_matches("0x");
    !text.is_empty() && text.len() <= 8 && text.chars().all(|c| c.is_ascii_hexdigit())
}

fn compile_menu(
    workbench: Option<&Value>,
    path_exists: &dyn Fn(&str) -> bool,
    errors: &mut Vec<String>,
) -> Option<Value> {
    let menu = workbench?.get("menu")?;
    // `enabled` is a historical editor artifact, not a build input. The
    // presence and validity of authored menu fields determine compilation.
    let mut values = Map::new();
    let title = menu
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if title.is_empty() {
        errors.push("menu.label: a menu entry needs a non-empty title".into());
    } else {
        values.insert("title".into(), json!(title));
    }
    let description = menu
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if !description.is_empty() {
        values.insert("description".into(), json!(description));
    }
    let mut categories: Vec<String> = Vec::new();
    for (index, entry) in menu
        .get("entries")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .enumerate()
    {
        let category = entry
            .get("categoryInstance")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if !hex_ok(category) {
            errors.push(format!(
                "menu.entries[{index}].categoryInstance: missing or invalid menu container"
            ));
            continue;
        }
        let hex = format!("0x{}", category.trim_start_matches("0x").to_uppercase());
        if !categories.contains(&hex) {
            categories.push(hex);
        }
    }
    if categories.is_empty() && !errors.iter().any(|e| e.starts_with("menu.entries")) {
        errors.push("menu.entries: at least one target menu container is required".into());
    }
    // Asset Editor's generic `dimension: city` entry is not the game's park
    // menu identity.  A workbench graph with a park leaf is the authoring
    // evidence for the runtime family, so complete the proven park recipe at
    // the interpreter boundary instead of requiring hidden game fields in UI.
    let has_park_behavior = workbench
        .and_then(|value| value.pointer("/logic/params"))
        .and_then(Value::as_object)
        .is_some_and(|params| {
            params.values().any(|value| {
                value.get("type").and_then(Value::as_str) == Some("park")
            })
        });
    values.insert("categoryInstances".into(), json!(categories));
    if has_park_behavior {
        values.insert("menuParentInstance".into(), json!("0xF970A822"));
        values.insert("buildingCategoryInstance".into(), json!("0xC710B6E9"));
    }

    if let Some(position) = menu.get("uiPosition").and_then(Value::as_i64) {
        values.insert("uiPosition".into(), json!(position));
    }
    for (key, field, label) in [
        ("iconPng", "iconPng", "menu.iconPng"),
        ("marqueePng", "marqueePng", "menu.marqueePng"),
    ] {
        let path = menu.get(key).and_then(Value::as_str).unwrap_or("").trim();
        if path.is_empty() {
            continue;
        }
        if !project_relative(path) {
            errors.push(format!("{label}: {path:?} must be a project-relative file"));
            continue;
        }
        if !path_exists(path) {
            errors.push(format!("{label}: file {path:?} does not exist"));
            continue;
        }
        let normalized = if field == "marqueePng"
            && path.to_ascii_lowercase().ends_with(".png")
        {
            let sibling = format!("{}jpg", &path[..path.len() - 3]);
            if path_exists(&sibling) {
                sibling
            } else {
                path.to_string()
            }
        } else {
            path.to_string()
        };
        values.insert(field.into(), json!(normalized));
    }
    for (key, field, label) in [
        (
            "constructionCost",
            "constructionCost",
            "menu.constructionCost",
        ),
        ("maintenanceCost", "maintenanceCost", "menu.maintenanceCost"),
    ] {
        if let Some(value) = menu.get(key).and_then(Value::as_i64) {
            if value < 0 {
                errors.push(format!("{label}: must be non-negative"));
                continue;
            }
            values.insert(field.into(), json!(value));
        }
    }
    // 临街吸附边：只写入 streetFacingDegrees，**不**写入 rotationDegrees。
    // rotationDegrees 是模型/场景几何朝向（Building40 打包用），与吸附边正交；
    // 由 schema/菜单显式声明，地块 tab 不得再把它当成临街选择。
    let street_facing = workbench
        .and_then(|value| value.get("streetFacingDegrees"))
        .or_else(|| menu.get("streetFacingDegrees"))
        .and_then(Value::as_u64);
    if let Some(degrees) = street_facing {
        if !matches!(degrees, 0 | 90 | 180 | 270) {
            errors.push(format!(
                "streetFacingDegrees: only 0/90/180/270 are supported (got {degrees})"
            ));
        } else {
            values.insert("streetFacingDegrees".into(), json!(degrees));
        }
    }
    Some(Value::Object(values))
}

fn project_relative(path: &str) -> bool {
    if path.starts_with('/') || path.contains("..") || path.starts_with('\\') {
        return false;
    }
    let bytes = path.as_bytes();
    !(bytes.len() >= 2 && bytes[1] == b':')
}

fn param_of(
    params: Option<&Map<String, Value>>,
    id: &str,
    errors: &mut Vec<String>,
) -> Option<Value> {
    let value = params
        .and_then(|params| params.get(id))
        .cloned()
        .unwrap_or(Value::Null);
    if value.is_null() || !value.is_object() {
        errors.push(format!(
            "logic node {id}: parameters are missing or invalid"
        ));
        None
    } else {
        Some(value)
    }
}

fn compile_logic(
    workbench: Option<&Value>,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> (Option<Value>, Vec<Value>, bool) {
    let Some(logic) = workbench.and_then(|workbench| workbench.get("logic")) else {
        return (None, Vec::new(), false);
    };
    let nodes = logic
        .get("nodes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if nodes.is_empty() {
        return (None, Vec::new(), false);
    }
    let edges = logic
        .get("edges")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let params = logic.get("params").and_then(Value::as_object);

    let mut ids = BTreeSet::new();
    let mut infos = Vec::new();
    let mut cores = Vec::new();
    let mut leaves = Vec::new();
    for node in nodes {
        let id = node.get("id").and_then(Value::as_str).unwrap_or("");
        let kind = node.get("kind").and_then(Value::as_str).unwrap_or("");
        if id.is_empty() {
            errors.push("logic.nodes: a node is missing its id".into());
            continue;
        }
        if !ids.insert(id.to_string()) {
            errors.push(format!("logic.nodes: duplicate node id {id}"));
            continue;
        }
        match kind {
            "building-info" => infos.push(id),
            "behavior-core" => cores.push(id),
            "behavior-leaf" => leaves.push(id),
            other => errors.push(format!("logic node {id}: unknown kind {other:?}")),
        }
    }
    if infos.len() > 1 {
        errors.push(format!(
            "logic: only one building-info node is allowed ({} found)",
            infos.len()
        ));
    }
    if cores.len() > 1 {
        errors.push(format!(
            "logic: only one behavior-core node is allowed ({} found)",
            cores.len()
        ));
    }
    if !leaves.is_empty() && cores.is_empty() {
        errors.push("logic: behavior leaves require a behavior-core node".into());
    }

    let mut attached_info = false;
    let mut attached_core = false;
    let mut attached_leaves = BTreeSet::new();
    for edge in edges {
        let source = edge.get("source").and_then(Value::as_str).unwrap_or("");
        let target = edge.get("target").and_then(Value::as_str).unwrap_or("");
        let handle = edge
            .get("targetHandle")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !ids.contains(source) {
            errors.push(format!("logic.edges: edge source {source:?} is not a node"));
            continue;
        }
        let source_is_leaf = leaves.iter().any(|leaf| *leaf == source);
        if source_is_leaf {
            let target_is_core = cores.iter().any(|core| *core == target);
            if handle != "leaf" || !target_is_core {
                errors.push(format!(
                    "logic edge from {source}: a behavior leaf must connect to the behavior-core leaf port"
                ));
            } else {
                attached_leaves.insert(source.to_string());
            }
        } else if target != "asset" {
            errors.push(format!(
                "logic edge from {source}: info/core nodes must connect to the asset anchor"
            ));
        } else if handle == "info" && infos.iter().any(|info| *info == source) {
            attached_info = true;
        } else if handle == "behavior" && cores.iter().any(|core| *core == source) {
            attached_core = true;
        } else {
            errors.push(format!(
                "logic edge from {source}: target port {handle:?} does not match the node kind"
            ));
        }
    }
    if !infos.is_empty() && !attached_info {
        errors.push("logic: building-info node is not attached to the asset anchor".into());
    }
    if !cores.is_empty() && !attached_core {
        errors.push("logic: behavior-core node is not attached to the asset anchor".into());
    }
    for leaf in &leaves {
        if !attached_leaves.contains(*leaf) {
            errors.push(format!(
                "logic leaf {leaf}: not attached to the behavior-core node"
            ));
        }
    }

    let mut labor = None;
    if let Some(id) = infos.first() {
        if let Some(node_params) = param_of(params, id, errors) {
            let field = |name: &str| -> Option<i64> {
                node_params
                    .get(name)
                    .and_then(Value::as_i64)
                    .filter(|v| *v >= 0)
            };
            // 主输入 = workers（这栋楼需要的劳工总数）；w1/w2/w3 是"按收入档
            // 拆分"的高级覆盖，minWorkers 留空由后端按原版比例自动推导。
            let workers = field("workers");
            let tiers = (field("w1"), field("w2"), field("w3"));
            let base = match (workers, tiers) {
                (Some(total), (Some(a), Some(b), Some(c))) => {
                    json!({ "workers": total, "w1": a, "w2": b, "w3": c })
                }
                (Some(total), _) => json!({ "workers": total }),
                (None, (Some(a), Some(b), Some(c))) => json!({ "w1": a, "w2": b, "w3": c }),
                _ => {
                    errors.push(format!(
                        "logic node {id}: workers (需要的劳工总数) must be a non-negative integer"
                    ));
                    Value::Null
                }
            };
            if !base.is_null() {
                let mut labor_value = base;
                if let Some(min) = field("minWorkers") {
                    labor_value["minWorkers"] = json!(min);
                }
                // 可选排班表（8 个非负整数=8 班位规模，0=关班；缺省由后端自动补齐）
                if let Some(schedule) = node_params
                    .get("schedule")
                    .and_then(Value::as_array)
                    .filter(|values| values.len() == 8)
                {
                    let mut values = Vec::with_capacity(8);
                    let mut valid = true;
                    for value in schedule {
                        match value.as_i64().filter(|v| (0..=999).contains(v)) {
                            Some(item) => values.push(item as i32),
                            None => {
                                errors.push(format!(
                                    "logic node {id}: schedule values must be integers 0-999"
                                ));
                                valid = false;
                                break;
                            }
                        }
                    }
                    if valid {
                        labor_value["schedule"] = json!(values);
                    }
                }
                labor = Some(labor_value);
            }
        }
    }

    let mut behaviors = Vec::new();
    for core in &cores {
        if let Some(node_params) = param_of(params, core, errors) {
            behaviors.push(json!({
                "id": core,
                "type": "utilities-core",
                "params": node_params
            }));
        }
    }
    for leaf in &leaves {
        let Some(node_params) = param_of(params, leaf, errors) else {
            continue;
        };
        let kind = node_params
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !matches!(
            kind,
            "education" | "tourism" | "park" | "tree" | "landvalue" | "residence"
        ) {
            errors.push(format!("logic leaf {leaf}: unknown behavior type {kind:?}"));
            continue;
        }
        behaviors.push(json!({"id": leaf, "type": kind, "params": node_params}));
    }

    // Asset Editor permits both tourism and park leaves to author the same
    // attraction field.  In the game this is one Unit property, not two
    // additive channels.  Tourism is the requested landmark behavior and is
    // therefore authoritative; retain the park node's land-value semantics
    // while dropping only its duplicate attraction write.
    let tourism_writes_attraction = behaviors.iter().any(|behavior| {
        behavior.get("type").and_then(Value::as_str) == Some("tourism")
            && behavior
                .pointer("/params/attraction")
                .is_some_and(|value| !value.is_null())
    });
    if tourism_writes_attraction {
        for behavior in &mut behaviors {
            if behavior.get("type").and_then(Value::as_str) != Some("park") {
                continue;
            }
            if behavior.pointer("/params/attraction").is_some() {
                if let Some(params) = behavior.get_mut("params").and_then(Value::as_object_mut) {
                    params.remove("attraction");
                    let id = behavior.get("id").and_then(Value::as_str).unwrap_or("park");
                    warnings.push(format!(
                        "behavior node {id}: park.attraction ignored because tourism.attraction is authoritative"
                    ));
                }
            }
        }
    }

    // 提前跑一遍行为编译：参数冲突/非法值在入队前点名节点。
    if errors.is_empty() {
        match crate::asset_build_m1::behavior_nodes::parse_labor(labor.as_ref()).and_then(|labor| {
            crate::asset_build_m1::behavior_nodes::parse_nodes(&behaviors)
                .and_then(|nodes| crate::asset_build_m1::behavior_nodes::compile(labor, &nodes))
        }) {
            Ok(resolved) => warnings.extend(resolved.notes),
            Err(error) => errors.push(error),
        }
    }
    (labor, behaviors, true)
}

/// 读取项目内场景文档，把工作台 menu/logic 覆盖进 schema。
///
/// 这是「所见即所得」的强制桥：e2e / CLI / 队列旁路若只喂裸 schema，
/// 会丢掉 `asset.lot.json` 里用户填写的 labor/地价/观光参数（/40 乱码根因）。
/// 文档缺失时返回 `Ok(None)`（保持 schema 原值）；文档存在但编译失败则 fail-closed。
pub fn overlay_document_on_schema(
    root: &Path,
    schema: &mut Value,
    document_rel: &str,
) -> Result<Option<CompiledLogic>, Vec<String>> {
    let document_path = root.join(document_rel);
    if !document_path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&document_path).map_err(|error| {
        vec![format!(
            "{}: failed to read workbench document: {error}",
            document_rel
        )]
    })?;
    let document: Value = serde_json::from_slice(&bytes).map_err(|error| {
        vec![format!(
            "{}: invalid workbench document JSON: {error}",
            document_rel
        )]
    })?;
    // Workbench assetPath is the live model binding after delete/re-import.
    // Schema files often keep a stale scene.model hash; force the document's
    // current path so packaging matches what the editor is showing.
    sync_workbench_model(schema, &document, root)?;
    let compiled = compile(&document, &|path| root.join(path).is_file())?;
    apply_to_schema(schema, &compiled)?;
    Ok(Some(compiled))
}

/// Copy `editor.workbench.assetPath` / `nativeLotm` onto `scene.model`.
fn sync_workbench_model(
    schema: &mut Value,
    document: &Value,
    root: &Path,
) -> Result<(), Vec<String>> {
    let workbench = match document.pointer("/editor/workbench") {
        Some(Value::Object(workbench)) => workbench,
        _ => return Ok(()),
    };
    if workbench.get("modelRemoved").and_then(Value::as_bool) == Some(true) {
        if let Some(scene) = schema.get_mut("scene").and_then(Value::as_object_mut) {
            scene.remove("model");
            scene.remove("modelShape");
        }
        return Ok(());
    }
    let model = workbench
        .get("assetPath")
        .or_else(|| workbench.get("nativeLotm"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty());
    let Some(model) = model else {
        return Ok(());
    };
    if !root.join(model).is_file() {
        return Err(vec![format!(
            "workbench model is missing: {model} (re-import the .blend/.lotm and save the workbench before building)"
        )]);
    }
    let Some(root_obj) = schema.as_object_mut() else {
        return Err(vec!["base schema must be a JSON object".into()]);
    };
    let scene = root_obj
        .entry("scene")
        .or_insert_with(|| json!({}));
    let Some(scene) = scene.as_object_mut() else {
        return Err(vec!["base schema scene must be an object".into()]);
    };
    scene.insert("model".into(), json!(model));
    scene.insert("modelShape".into(), json!("building40"));
    if scene
        .get("document")
        .and_then(Value::as_str)
        .is_none_or(|value| value.trim().is_empty())
    {
        scene.insert("document".into(), json!("asset.lot.json"));
    }
    Ok(())
}

/// 把编译结果覆盖到基础 schema 的 `semantics`（其余段原样保留）。
/// 有 `scene.model` 时强制 `scene.modelShape=building40`（生产路径钉死；
/// 工作台只编排场景/逻辑，建筑编译走 Rust Building40 内核）。
pub fn apply_to_schema(base: &mut Value, compiled: &CompiledLogic) -> Result<(), Vec<String>> {
    let Some(root) = base.as_object_mut() else {
        return Err(vec!["base schema must be a JSON object".into()]);
    };
    let semantics = root.entry("semantics").or_insert_with(|| json!({}));
    let Some(semantics) = semantics.as_object_mut() else {
        return Err(vec!["base schema semantics must be an object".into()]);
    };
    let legacy_behavior = semantics
        .get("behavior")
        .and_then(Value::as_object)
        .is_some_and(|behavior| !behavior.is_empty());
    if compiled.logic_present && legacy_behavior {
        return Err(vec![
            "semantics.behavior (legacy) conflicts with the workbench logic graph; \
             migrate the binding to logic nodes"
                .into(),
        ]);
    }
    if let Some(menu) = &compiled.menu {
        semantics.insert("menu".into(), menu.clone());
    }
    if compiled.logic_present {
        match &compiled.labor {
            Some(labor) => {
                semantics.insert("labor".into(), labor.clone());
            }
            None => {
                semantics.remove("labor");
            }
        }
        semantics.insert("behaviors".into(), json!(compiled.behaviors));
    }
    // Old workbench schemas can request a delta even after the document adds
    // simulation behavior to a new menu identity. The behavior processor
    // requires a standalone package for that combination.
    if compiled.menu.is_some() && (compiled.labor.is_some() || !compiled.behaviors.is_empty()) {
        if let Some(outputs) = root.get_mut("outputs").and_then(Value::as_object_mut) {
            outputs.insert("mode".into(), json!("full"));
        }
    }
    pin_building40_model_shape(root)?;
    Ok(())
}

fn pin_building40_model_shape(root: &mut Map<String, Value>) -> Result<(), Vec<String>> {
    let Some(scene) = root.get_mut("scene").and_then(Value::as_object_mut) else {
        return Ok(());
    };
    let has_model = scene
        .get("model")
        .and_then(Value::as_str)
        .is_some_and(|path| !path.trim().is_empty());
    if !has_model {
        return Ok(());
    }
    if let Some(shape) = scene.get("modelShape").and_then(Value::as_str) {
        if shape != "building40" {
            return Err(vec![format!(
                "scene.modelShape {shape:?} is forbidden for workbench builds; \
                 production packaging uses Rust Building40 only"
            )]);
        }
    }
    scene.insert("modelShape".into(), json!("building40"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(menu: Value, logic: Value) -> Value {
        json!({"editor": {"workbench": {"menu": menu, "logic": logic}}})
    }

    fn menu(on: bool) -> Value {
        let binding = json!({
            "enabled": on,
            "label": "科洛大厦",
            "description": "测试",
            "uiPosition": 12000,
            "iconPng": "",
            "marqueePng": "",
            "constructionCost": null,
            "maintenanceCost": null,
            "entries": [
                {"id": "menu-1", "dimension": "city", "categoryInstance": "899BEA70"},
                {"id": "menu-2", "dimension": "development", "categoryInstance": "94E412ED"}
            ]
        });
        if !on {
            // Legacy helper for an empty menu object; `enabled` itself is not
            // interpreted by the production compiler.
            let mut empty = binding;
            empty["label"] = json!("");
            empty["description"] = json!("");
            empty["uiPosition"] = json!(null);
            empty["entries"] = json!([]);
            return empty;
        }
        binding
    }

    fn empty_logic() -> Value {
        json!({"nodes": [], "edges": [], "params": {}})
    }

    #[test]
    fn old_delta_schema_with_new_menu_behavior_builds_a_standalone_package() {
        let mut schema = json!({"outputs": {"mode": "delta", "package": "test.package"}});
        let compiled = CompiledLogic {
            menu: Some(json!({"title": "Test"})),
            labor: Some(json!({"workers": 5})),
            behaviors: Vec::new(),
            logic_present: true,
            warnings: Vec::new(),
        };
        apply_to_schema(&mut schema, &compiled).unwrap();
        assert_eq!(schema["outputs"]["mode"], "full");
        assert_eq!(schema["outputs"]["package"], "test.package");
    }

    #[test]
    fn compiles_street_facing_without_model_rotation() {
        let mut doc = document(menu(true), empty_logic());
        doc["editor"]["workbench"]["streetFacingDegrees"] = json!(90);
        let compiled = compile(&doc, &|_| true).unwrap();
        let values = compiled.menu.expect("menu");
        assert_eq!(values["streetFacingDegrees"], 90);
        assert!(values.get("rotationDegrees").is_none());
    }

    #[test]
    fn rejects_invalid_street_facing_degrees() {
        let mut doc = document(menu(true), empty_logic());
        doc["editor"]["workbench"]["streetFacingDegrees"] = json!(45);
        let errors = compile(&doc, &|_| true).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("streetFacingDegrees")));
    }

    #[test]
    fn compiles_menu_binding_to_semantics_menu() {
        let doc = document(menu(true), empty_logic());
        let compiled = compile(&doc, &|_| true).unwrap();
        let values = compiled.menu.unwrap();
        assert_eq!(values["title"], "科洛大厦");
        assert_eq!(
            values["categoryInstances"],
            json!(["0x899BEA70", "0x94E412ED"])
        );
        assert_eq!(values["uiPosition"], 12000);
    }

    #[test]
    fn park_behavior_with_labor_preserves_selected_menu_destinations() {
        let workbench = json!({
            "menu": menu(true),
            "logic": {"params": {"park": {"type": "park"}, "labor": {"w1": 20}}}
        });
        let mut errors = Vec::new();
        let values = compile_menu(Some(&workbench), &|_| true, &mut errors).unwrap();
        assert!(errors.is_empty());
        assert_eq!(
            values["categoryInstances"],
            json!(["0x899BEA70", "0x94E412ED"])
        );
        assert_eq!(values["menuParentInstance"], "0xF970A822");
    }

    #[test]
    fn absent_menu_leaves_the_base_schema_untouched() {
        let doc = json!({"editor": {"workbench": {"logic": empty_logic()}}});
        let compiled = compile(&doc, &|_| true).unwrap();
        assert!(compiled.menu.is_none());
        assert!(!compiled.logic_present);
    }

    #[test]
    fn menu_configuration_is_preserved_without_an_enable_switch() {
        // `enabled` is ignored; authored menu data remains authoritative.
        let mut binding = menu(true);
        binding["enabled"] = json!(false);
        let compiled = compile(&document(binding, empty_logic()), &|_| true).unwrap();
        let menu = compiled.menu.expect("authored menu preserved");
        assert!(
            menu["categoryInstances"]
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value == "0x899BEA70"))
        );
    }

    #[test]
    fn missing_menu_title_and_categories_are_reported() {
        let mut binding = menu(true);
        binding["label"] = json!("  ");
        binding["entries"] = json!([]);
        let errors = compile(&document(binding, empty_logic()), &|_| true).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("title")), "{errors:?}");
        assert!(
            errors.iter().any(|e| e.contains("menu container")),
            "{errors:?}"
        );
    }

    #[test]
    fn menu_image_paths_must_exist_in_the_project() {
        let mut binding = menu(true);
        binding["iconPng"] = json!("assets/menu/icon.png");
        let errors = compile(&document(binding, empty_logic()), &|_| false).unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("does not exist")),
            "{errors:?}"
        );
    }

    #[test]
    fn compiles_labor_and_behavior_nodes() {
        let logic = json!({
            "nodes": [
                {"id": "info-1", "kind": "building-info", "position": [0, 0]},
                {"id": "core-1", "kind": "behavior-core", "position": [0, 100]},
                {"id": "leaf-education-1", "kind": "behavior-leaf", "position": [0, 200]}
            ],
            "edges": [
                {"id": "e1", "source": "info-1", "target": "asset", "targetHandle": "info"},
                {"id": "e2", "source": "core-1", "target": "asset", "targetHandle": "behavior"},
                {"id": "e3", "source": "leaf-education-1", "target": "core-1", "targetHandle": "leaf"}
            ],
            "params": {
                "info-1": {"w1": 20, "w2": 5, "w3": 0, "minWorkers": 5},
                "core-1": {
                    "power": {"mode": "inherit", "rate": 72, "amount": null},
                    "water": {"mode": "inherit", "rate": null, "amount": null},
                    "garbage": {"mode": "inherit", "rate": null, "amount": null}
                },
                "leaf-education-1": {"type": "education", "capacity": 250, "walkPath": true}
            }
        });
        let compiled = compile(&json!({"editor": {"workbench": {"logic": logic}}}), &|_| {
            true
        })
        .unwrap();
        assert!(compiled.logic_present);
        assert_eq!(compiled.labor.unwrap()["w1"], 20);
        assert_eq!(compiled.behaviors.len(), 2);
        assert_eq!(compiled.behaviors[1]["type"], "education");
    }

    #[test]
    fn building_info_accepts_the_single_intuitive_workers_field() {
        // 2026-11-11 用户指令：前端的直觉输入只有「需要的劳工数」，
        // 分档/工人槽/开工阈值由后端语义层推导。
        let logic = json!({
            "nodes": [{"id": "info-1", "kind": "building-info", "position": [0, 0]}],
            "edges": [{"id": "e1", "source": "info-1", "target": "asset", "targetHandle": "info"}],
            "params": {"info-1": {"workers": 30}}
        });
        let compiled = compile(&json!({"editor": {"workbench": {"logic": logic}}}), &|_| {
            true
        })
        .unwrap();
        let labor = compiled.labor.unwrap();
        assert_eq!(labor["workers"], 30);
        assert!(labor.get("w1").is_none(), "分档由 parse_labor 推导");
        assert!(labor.get("minWorkers").is_none(), "阈值由 parse_labor 推导");

        let spec = super::super::behavior_nodes::parse_labor(Some(&labor))
            .unwrap()
            .unwrap();
        assert_eq!((spec.w1, spec.w2, spec.w3), (30, 0, 0));
        assert_eq!(spec.min_workers, 5);
    }

    #[test]
    fn unattached_leaf_is_rejected() {
        let logic = json!({
            "nodes": [
                {"id": "core-1", "kind": "behavior-core", "position": [0, 0]},
                {"id": "leaf-1", "kind": "behavior-leaf", "position": [0, 100]}
            ],
            "edges": [
                {"id": "e1", "source": "core-1", "target": "asset", "targetHandle": "behavior"}
            ],
            "params": {
                "core-1": {
                    "power": {"mode": "inherit"},
                    "water": {"mode": "inherit"},
                    "garbage": {"mode": "inherit"}
                },
                "leaf-1": {"type": "education", "capacity": 100}
            }
        });
        let errors = compile(&json!({"editor": {"workbench": {"logic": logic}}}), &|_| {
            true
        })
        .unwrap_err();
        assert!(errors.iter().any(|e| e.contains("leaf-1")), "{errors:?}");
    }

    #[test]
    fn overlay_replaces_stale_schema_model_with_workbench_asset_path() {
        let dir = std::env::temp_dir().join(format!(
            "openscp-wb-model-sync-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("assets/models")).unwrap();
        let model = "assets/models/fresh.lotm";
        std::fs::write(dir.join(model), b"LOTM").unwrap();
        std::fs::write(
            dir.join("asset.lot.json"),
            serde_json::to_vec_pretty(&json!({
                "editor": {
                    "workbench": {
                        "source": "lotm",
                        "assetPath": model,
                        "menu": menu(true),
                        "logic": empty_logic()
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let mut schema = json!({
            "scene": {
                "document": "asset.lot.json",
                "model": "assets/models/stale.lotm",
                "modelShape": "building40"
            },
            "semantics": {}
        });
        overlay_document_on_schema(&dir, &mut schema, "asset.lot.json").unwrap();
        assert_eq!(schema["scene"]["model"], model);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn deleted_model_clears_stale_schema_binding() {
        let mut schema = json!({
            "scene": {
                "document": "asset.lot.json",
                "model": "assets/models/deleted.lotm",
                "modelShape": "building40",
                "writes": ["props:0", "lights"]
            }
        });
        let document = json!({"editor": {"workbench": {
            "source": "none",
            "modelRemoved": true
        }}});
        sync_workbench_model(&mut schema, &document, Path::new(".")).unwrap();
        assert_eq!(schema["scene"], json!({
            "document": "asset.lot.json",
            "writes": ["props:0", "lights"]
        }));
    }

    #[test]
    fn apply_pins_building40_when_scene_model_present() {
        let mut base = json!({
            "$schema": "openscp.asset",
            "schema_version": 2,
            "id": "demo",
            "kind": "building-asset",
            "reference": {},
            "scene": {"document": "asset.lot.json", "model": "models/a.lotm"},
            "semantics": {},
            "outputs": {"package": "demo.package"}
        });
        let compiled = CompiledLogic::default();
        apply_to_schema(&mut base, &compiled).unwrap();
        assert_eq!(base["scene"]["modelShape"], "building40");

        base["scene"]["modelShape"] = json!("nh-atlas");
        let error = apply_to_schema(&mut base, &compiled).unwrap_err();
        assert!(error[0].contains("Building40"), "{error:?}");
    }

    #[test]
    fn apply_overlays_semantics_and_keeps_scene() {
        let doc = document(menu(true), empty_logic());
        let compiled = compile(&doc, &|_| true).unwrap();
        let mut base = json!({
            "reference": {"package": "source.package"},
            "scene": {"document": "asset.lot.json"},
            "semantics": {"lotChange": true}
        });
        apply_to_schema(&mut base, &compiled).unwrap();
        assert_eq!(base["semantics"]["menu"]["title"], "科洛大厦");
        assert_eq!(base["scene"]["document"], "asset.lot.json");
        assert_eq!(base["reference"]["package"], "source.package");
        assert_eq!(base["semantics"]["lotChange"], true);
    }

    #[test]
    fn legacy_behavior_conflicts_with_logic_graph() {
        let logic = json!({
            "nodes": [{"id": "info-1", "kind": "building-info", "position": [0, 0]}],
            "edges": [{"id": "e1", "source": "info-1", "target": "asset", "targetHandle": "info"}],
            "params": {"info-1": {"w1": 1, "w2": 0, "w3": 0, "minWorkers": 0}}
        });
        let compiled = compile(&json!({"editor": {"workbench": {"logic": logic}}}), &|_| {
            true
        })
        .unwrap();
        let mut base = json!({"semantics": {"behavior": {"kind": "education-minimal"}}});
        let errors = apply_to_schema(&mut base, &compiled).unwrap_err();
        assert!(errors[0].contains("conflicts"), "{errors:?}");
    }
}
