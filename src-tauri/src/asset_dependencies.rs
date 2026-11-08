//! 工作台文档 → package.json `dependencies` 同步（2026-11-07）。
//!
//! 设计文档 §2.2 / §6 验收第 1 条："画一条人行道即登记
//! `resources.pathEntries += 418A18B1`"。本模块是那次登记的**唯一换算出口**：
//!
//! - 输入：工作台文档（`asset.lot.json`）的 `editor.workbench.paths[].type`；
//! - 输出：`dependencies.resources.pathEntries`（排序去重的 entry hex）；
//! - 副作用：只由 `sync_package_dependencies` 写一次 package.json（原子写）。
//!
//! 纯函数部分（`derive` / `merge`）不碰文件系统，可单测；命令层只做
//! 读 package.json → merge → 原子写回。

use crate::activity::{AppState, CommandError};
use serde_json::{Value, json};
use std::path::Path;
use std::sync::Arc;
use tauri::State;

const MANIFEST: &str = "package.json";
const MANIFEST_FORMAT: &str = "openscp.mod";

/// 文档里的路径实体 → 依赖清单（纯函数）。
///
/// 解析失败（未知类型/坏 entry）计入 `warnings`，不阻断保存——构建阶段
/// 的 validate-paths 才是 fail-closed 的地方；这里只保证"账本不撒谎"。
pub fn derive(document: &Value) -> (Value, Vec<String>) {
    let mut warnings = Vec::new();
    let mut path_entries: Vec<String> = Vec::new();
    if let Some(paths) = document
        .pointer("/editor/workbench/paths")
        .and_then(Value::as_array)
    {
        for (index, path) in paths.iter().enumerate() {
            let kind = path
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|kind| !kind.is_empty());
            let entry = path
                .get("entry")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|entry| !entry.is_empty());
            let scope_local = path
                .get("scope")
                .and_then(Value::as_str)
                .map(|scope| scope != "global")
                .unwrap_or(true);
            let road_connect = path
                .get("roadConnect")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            match crate::asset_build_m1::path_catalog::resolve(
                kind,
                entry,
                scope_local,
                road_connect,
            ) {
                Ok(recipe) => {
                    let hex = format!("0x{:08X}", recipe.entry_instance);
                    if !path_entries.contains(&hex) {
                        path_entries.push(hex);
                    }
                }
                Err(error) => warnings.push(format!("paths[{index}]: {error}")),
            }
        }
    }
    path_entries.sort();

    // 菜单编排登记（设计文档 §2.2 dependencies.menu + statics）：
    // 标题/容器入账，图标与背景图作为静态资源登记（role 区分用途）。
    let mut menu = Value::Null;
    let mut statics: Vec<Value> = Vec::new();
    if let Some(binding) = document.pointer("/editor/workbench/menu") {
        if binding.get("enabled").and_then(Value::as_bool) == Some(true) {
            let title = binding
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let categories: Vec<String> = binding
                .get("entries")
                .and_then(Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| {
                            entry
                                .get("categoryInstance")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|id| !id.is_empty())
                                .map(|id| {
                                    format!("0x{}", id.trim_start_matches("0x").to_uppercase())
                                })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let mut values = json!({"title": title, "categories": categories});
            if let Some(position) = binding.get("uiPosition").and_then(Value::as_i64) {
                values["uiPosition"] = json!(position);
            }
            menu = values;
            for (field, role) in [("iconPng", "menu-icon"), ("marqueePng", "menu-marquee")] {
                if let Some(path) = binding
                    .get(field)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                {
                    statics.push(json!({"path": path, "role": role}));
                }
            }
        }
    }

    let mut derived = json!({
        "resources": {
            "pathEntries": path_entries,
        },
    });
    if !menu.is_null() {
        derived["menu"] = menu;
    }
    if !statics.is_empty() {
        derived["statics"] = json!(statics);
    }
    (derived, warnings)
}

/// 把派生清单并进 package.json 的 `dependencies`（纯函数）。
///
/// 不覆盖已有键：`dependencies` 里其它资源族（bundles/props/lots/menu…）
/// 由各自的提取器维护，本模块只更新 `resources.pathEntries`。
pub fn merge(manifest: &mut Value, derived: &Value) -> Value {
    let root = manifest
        .as_object_mut()
        .expect("manifest must be a JSON object");
    let dependencies = root.entry("dependencies").or_insert_with(|| json!({}));
    if !dependencies.is_object() {
        *dependencies = json!({});
    }
    let resources = dependencies
        .as_object_mut()
        .expect("dependencies must be an object")
        .entry("resources")
        .or_insert_with(|| json!({}));
    if !resources.is_object() {
        *resources = json!({});
    }
    let resources = resources.as_object_mut().expect("resources object");
    if let Some(path_entries) = derived.pointer("/resources/pathEntries") {
        resources.insert("pathEntries".into(), path_entries.clone());
    }
    let dependencies = root
        .get_mut("dependencies")
        .and_then(Value::as_object_mut)
        .expect("dependencies object");
    if let Some(menu) = derived.get("menu") {
        dependencies.insert("menu".into(), menu.clone());
    }
    if let Some(statics) = derived.get("statics").and_then(Value::as_array) {
        let existing = dependencies.entry("statics").or_insert_with(|| json!([]));
        if !existing.is_array() {
            *existing = json!([]);
        }
        let list = existing.as_array_mut().expect("statics array");
        for entry in statics {
            let path = entry.get("path").and_then(Value::as_str).unwrap_or("");
            list.retain(|item| item.get("path").and_then(Value::as_str) != Some(path));
            list.push(entry.clone());
        }
    }
    Value::Object(dependencies.clone())
}

/// 命令入口：读 package.json → merge → 原子写回。返回合并后的 dependencies。
pub fn sync_project(root: &Path, document: &Value) -> Result<Value, String> {
    let manifest_path = root.join(MANIFEST);
    let bytes = std::fs::read(&manifest_path).map_err(|error| {
        format!("package.json: {error} (asset dependency sync needs a mod project)")
    })?;
    let mut manifest: Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("package.json: {error}"))?;
    if manifest.get("format").and_then(Value::as_str) != Some(MANIFEST_FORMAT) {
        return Err("package.json is not an OpenSCP mod manifest".into());
    }
    let (derived, warnings) = derive(document);
    let dependencies = merge(&mut manifest, &derived);
    let encoded =
        serde_json::to_vec_pretty(&manifest).map_err(|error| format!("package.json: {error}"))?;
    crate::atomic_fs::write_atomic(&manifest_path, &encoded, true)
        .map_err(|error| format!("package.json write: {error}"))?;
    let mut out = dependencies;
    if let Some(object) = out.as_object_mut() {
        object.insert("warnings".into(), json!(warnings));
    }
    Ok(out)
}

/// `project` 是 mod 项目相对 modRoot 的路径（与其余 `code_*` 命令一致），
/// 由 `workspace::code_root` 经 studio 配置解析——不是裸文件系统路径。
#[tauri::command]
pub async fn sync_asset_dependencies(
    state: State<'_, AppState>,
    project: String,
    document: String,
) -> Result<Value, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let document: Value =
            serde_json::from_str(&document).map_err(|error| format!("document: {error}"))?;
        let root = crate::workspace::code_root(&store, &project)
            .map_err(|error| format!("project: {error}"))?;
        sync_project(&root, &document)
    })
    .await
    .map_err(|error| CommandError::new("asset_dependencies_failed", error.to_string()))?
    .map_err(|error| CommandError::new("asset_dependencies_failed", error))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(paths: Value) -> Value {
        json!({"editor": {"workbench": {"paths": paths}}})
    }

    #[test]
    fn pedestrian_registers_vanilla_entry() {
        let (derived, warnings) = derive(&document(json!([
            {"id": "path:1", "type": "pedestrian", "pointIds": []}
        ])));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(derived["resources"]["pathEntries"], json!(["0x418A18B1"]));
    }

    #[test]
    fn entries_are_deduplicated_and_sorted() {
        let (derived, _) = derive(&document(json!([
            {"id": "path:1", "type": "driveway"},
            {"id": "path:2", "type": "pedestrian"},
            {"id": "path:3", "type": "pedestrian"}
        ])));
        assert_eq!(
            derived["resources"]["pathEntries"],
            json!(["0x418A18B1", "0x7C9C4FFF"])
        );
    }

    #[test]
    fn custom_entry_is_recorded_and_invalid_type_warns() {
        let (derived, warnings) = derive(&document(json!([
            {"id": "path:1", "type": "custom", "entry": "675b61b1"},
            {"id": "path:2", "type": "unknown"}
        ])));
        assert_eq!(derived["resources"]["pathEntries"], json!(["0x675B61B1"]));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("paths[1]"), "{warnings:?}");
    }

    #[test]
    fn menu_binding_registers_dependencies_menu_and_static_assets() {
        let document = json!({
            "editor": {"workbench": {
                "paths": [],
                "menu": {
                    "enabled": true,
                    "label": "科洛大厦",
                    "uiPosition": 12000,
                    "iconPng": "assets/menu/icon.png",
                    "marqueePng": "assets/menu/marquee.jpg",
                    "entries": [
                        {"id": "menu-1", "dimension": "city", "categoryInstance": "899bea70"}
                    ]
                }
            }}
        });
        let (derived, warnings) = derive(&document);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(derived["menu"]["title"], "科洛大厦");
        assert_eq!(derived["menu"]["categories"], json!(["0x899BEA70"]));
        assert_eq!(
            derived["statics"],
            json!([
                {"path": "assets/menu/icon.png", "role": "menu-icon"},
                {"path": "assets/menu/marquee.jpg", "role": "menu-marquee"}
            ])
        );
    }

    #[test]
    fn merge_upserts_menu_and_dedupes_statics() {
        let mut manifest = json!({
            "format": "openscp.mod",
            "dependencies": {
                "menu": {"title": "old"},
                "statics": [{"path": "assets/menu/icon.png", "role": "stale"}]
            }
        });
        let (derived, _) = derive(&json!({
            "editor": {"workbench": {"menu": {
                "enabled": true,
                "label": "新标题",
                "entries": [{"id": "menu-1", "dimension": "city", "categoryInstance": "94E412ED"}],
                "iconPng": "assets/menu/icon.png"
            }}}
        }));
        let merged = merge(&mut manifest, &derived);
        assert_eq!(merged["menu"]["title"], "新标题");
        let statics = merged["statics"].as_array().unwrap();
        assert_eq!(statics.len(), 1, "same path is replaced, not duplicated");
        assert_eq!(statics[0]["role"], "menu-icon");
    }

    #[test]
    fn merge_keeps_other_dependency_families() {
        let mut manifest = json!({
            "format": "openscp.mod",
            "dependencies": {"resources": {"bundles": ["0x6C042366"]}}
        });
        let (derived, _) = derive(&document(json!([
            {"id": "path:1", "type": "pedestrian"}
        ])));
        let merged = merge(&mut manifest, &derived);
        assert_eq!(merged["resources"]["bundles"], json!(["0x6C042366"]));
        assert_eq!(merged["resources"]["pathEntries"], json!(["0x418A18B1"]));
    }
}
