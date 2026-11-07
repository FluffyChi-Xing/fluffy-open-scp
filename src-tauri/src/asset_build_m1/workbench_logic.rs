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

use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

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
    if menu.get("enabled").and_then(Value::as_bool) != Some(true) {
        return None;
    }
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
    values.insert("categoryInstances".into(), json!(categories));

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
            errors.push(format!(
                "{label}: {path:?} must be a project-relative file"
            ));
            continue;
        }
        if !path_exists(path) {
            errors.push(format!("{label}: file {path:?} does not exist"));
            continue;
        }
        values.insert(field.into(), json!(path));
    }
    for (key, field, label) in [
        ("constructionCost", "constructionCost", "menu.constructionCost"),
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
        errors.push(format!("logic node {id}: parameters are missing or invalid"));
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
            match (
                field("w1"),
                field("w2"),
                field("w3"),
                field("minWorkers"),
            ) {
                (Some(w1), Some(w2), Some(w3), Some(min_workers)) => {
                    let mut labor_value = json!({
                        "w1": w1,
                        "w2": w2,
                        "w3": w3,
                        "minWorkers": min_workers,
                    });
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
                _ => errors.push(format!(
                    "logic node {id}: w1/w2/w3/minWorkers must be non-negative integers"
                )),
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
        if !matches!(kind, "education" | "tourism" | "park" | "tree" | "landvalue") {
            errors.push(format!("logic leaf {leaf}: unknown behavior type {kind:?}"));
            continue;
        }
        behaviors.push(json!({"id": leaf, "type": kind, "params": node_params}));
    }

    // 提前跑一遍行为编译：参数冲突/非法值在入队前点名节点。
    if errors.is_empty() {
        match crate::asset_build_m1::behavior_nodes::parse_labor(labor.as_ref())
            .and_then(|labor| {
                crate::asset_build_m1::behavior_nodes::parse_nodes(&behaviors)
                    .and_then(|nodes| {
                        crate::asset_build_m1::behavior_nodes::compile(labor, &nodes)
                    })
            }) {
            Ok(resolved) => warnings.extend(resolved.notes),
            Err(error) => errors.push(error),
        }
    }
    (labor, behaviors, true)
}

/// 把编译结果覆盖到基础 schema 的 `semantics`（其余段原样保留）。
pub fn apply_to_schema(base: &mut Value, compiled: &CompiledLogic) -> Result<(), Vec<String>> {
    let Some(root) = base.as_object_mut() else {
        return Err(vec!["base schema must be a JSON object".into()]);
    };
    let semantics = root
        .entry("semantics")
        .or_insert_with(|| json!({}));
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(menu: Value, logic: Value) -> Value {
        json!({"editor": {"workbench": {"menu": menu, "logic": logic}}})
    }

    fn menu(on: bool) -> Value {
        json!({
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
        })
    }

    fn empty_logic() -> Value {
        json!({"nodes": [], "edges": [], "params": {}})
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
    fn disabled_menu_leaves_the_base_schema_untouched() {
        let doc = document(menu(false), empty_logic());
        let compiled = compile(&doc, &|_| true).unwrap();
        assert!(compiled.menu.is_none());
        assert!(!compiled.logic_present);
    }

    #[test]
    fn missing_menu_title_and_categories_are_reported() {
        let mut binding = menu(true);
        binding["label"] = json!("  ");
        binding["entries"] = json!([]);
        let errors = compile(&document(binding, empty_logic()), &|_| true).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("title")), "{errors:?}");
        assert!(errors.iter().any(|e| e.contains("menu container")), "{errors:?}");
    }

    #[test]
    fn menu_image_paths_must_exist_in_the_project() {
        let mut binding = menu(true);
        binding["iconPng"] = json!("assets/menu/icon.png");
        let errors = compile(&document(binding, empty_logic()), &|_| false).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("does not exist")), "{errors:?}");
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
        let compiled = compile(&document(menu(false), logic), &|_| true).unwrap();
        assert!(compiled.logic_present);
        assert_eq!(compiled.labor.unwrap()["w1"], 20);
        assert_eq!(compiled.behaviors.len(), 2);
        assert_eq!(compiled.behaviors[1]["type"], "education");
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
        let errors = compile(&document(menu(false), logic), &|_| true).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("leaf-1")), "{errors:?}");
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
        let compiled = compile(&document(menu(false), logic), &|_| true).unwrap();
        let mut base = json!({"semantics": {"behavior": {"kind": "education-minimal"}}});
        let errors = apply_to_schema(&mut base, &compiled).unwrap_err();
        assert!(errors[0].contains("conflicts"), "{errors:?}");
    }
}
