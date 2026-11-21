use super::{
    components, hash, item_name, manifest, parse, sources,
    spec::{MissingFeedback, PanelPlan},
};
use crate::asset_build_m1::{
    filesystem::project_file,
    patch::parse_tgi,
    plan::PropertyBuildPlan,
    property_edit::{encode, set_array, set_scalar},
    stages::BuildContext,
};
use dbpf::ResourceId;
use sc_properties::{Key, PropType, PropertyFile, Value};
use serde_json::json;
use std::{fs, path::Path};

/// Resolve before any run-cache lookup: panel-only edits must not replay old bytes.
pub(crate) fn attach(root: &Path, plan: &mut PropertyBuildPlan) -> Result<(), String> {
    let manifest_path = root.join("package.json");
    if !manifest_path.is_file() {
        return Ok(());
    }
    let m: serde_json::Value =
        serde_json::from_slice(&fs::read(manifest_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let Some(dependency) = manifest::dependency(&m)? else {
        return Ok(());
    };
    if plan.menu.is_none() {
        return Err("PANEL_NEW_MENU_IDENTITY_REQUIRED".into());
    }
    let raw =
        fs::read_to_string(project_file(root, &dependency.schema)?).map_err(|e| e.to_string())?;
    let config = parse(&raw)?;
    let leaves: Vec<&str> = plan
        .intermediate
        .semantics
        .behaviors
        .iter()
        .filter_map(|v| v["type"].as_str())
        .collect();
    for id in config.hover.iter().chain(&config.click) {
        let c = components()
            .into_iter()
            .find(|c| &c.id == id)
            .expect("validated component");
        if c.group != "common" && !leaves.contains(&c.group.as_str()) {
            return Err(format!("PANEL_BEHAVIOR_REQUIRED:{}:{}", c.id, c.group));
        }
    }
    sources::open(root, &dependency.sources)?;
    let fingerprint = sources::digest(
        &serde_json::to_vec(&(&config, &dependency, super::RUNTIME, components()))
            .map_err(|e| e.to_string())?,
    );
    plan.panel = Some(PanelPlan {
        path: dependency.schema.clone(),
        config,
        dependency,
        fingerprint,
    });
    Ok(())
}

fn ids(items: &[String], hide: bool) -> Vec<Value> {
    if items.is_empty() {
        return vec![Value::UInt32(hash("OpenSCPPanel_empty"))];
    }
    let catalog = components();
    items
        .iter()
        .map(|id| {
            let c = catalog.iter().find(|c| &c.id == id).unwrap();
            let name = if c.feedback {
                format!(
                    "{}{}",
                    item_name(id),
                    if hide { "_hide" } else { "_native" }
                )
            } else {
                item_name(id)
            };
            Value::UInt32(hash(&name))
        })
        .collect()
}

pub(crate) fn apply(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    let Some(panel) = ctx.plan.panel.as_ref() else {
        return Ok(serde_json::Value::Null);
    };
    let instance = ctx
        .menu_instance
        .ok_or("PANEL_NEW_MENU_IDENTITY_REQUIRED")?;
    let packages = sources::open(&ctx.root, &panel.dependency.sources)?;
    let source_id = parse_tgi(&panel.config.shell)?;
    let mut shell: serde_json::Value =
        serde_json::from_slice(&sources::read(&packages, source_id)?)
            .map_err(|e| format!("PANEL_LAYOUT:{e}"))?;
    if shell["type"] != "cLayout" {
        return Err("PANEL_LAYOUT_TYPE".into());
    }
    for control in [
        164191353, 212238278, 252147891, 215183945, 215191523, 226403367, 248008263,
    ] {
        if !has_control(&shell, control) {
            return Err(format!("PANEL_SHELL_CONTROL:{control:08X}"));
        }
    }
    clean_zot(&mut shell);
    let layout = ResourceId {
        instance: hash(&format!("OpenSCPPanelShell_{instance:08X}")),
        ..source_id
    };
    if ctx.resources.contains_key(&layout) {
        return Err("PANEL_LAYOUT_ID_COLLISION".into());
    }
    let source_game = sources::read(&packages, sources::GAME_JS)?;
    let source_game = std::str::from_utf8(&source_game).map_err(|e| e.to_string())?;
    if source_game.contains("OpenSCPPanelRuntimeVersion") {
        return Err("PANEL_RUNTIME_SOURCE_ALREADY_PATCHED".into());
    }
    for c in components() {
        if !source_game.contains(&format!("simcity.RolloverItems.{}", c.native)) {
            return Err(format!("PANEL_NATIVE_BINDING:{}", c.native));
        }
    }
    let suffix = format!(
        "\n;window.OpenSCPPanelComponents={};\n{}",
        serde_json::to_string(&components()).map_err(|e| e.to_string())?,
        super::RUNTIME
    );
    let runtime = [source_game.as_bytes(), suffix.as_bytes()].concat();
    // Shared, deterministic runtime bytes across every panel from this source/version.
    // No installed file is edited. The UI overlay is an explicit output dependency.
    if ctx
        .resources
        .get(&sources::GAME_JS)
        .is_some_and(|old| old.as_slice() != source_game.as_bytes() && old != &runtime)
    {
        return Err("PANEL_RUNTIME_OVERLAY_CONFLICT".into());
    }
    let asset_id = ResourceId {
        type_id: 0x00B1_B104,
        group: 0x40E1_C000,
        instance,
    };
    let mut asset = PropertyFile::parse(ctx.resources.get(&asset_id).ok_or("PANEL_ASSET_MISSING")?)
        .map_err(|e| e.to_string())?;
    let hide = panel.config.missing_feedback == MissingFeedback::Hide;
    set_scalar(
        &mut asset,
        0x0BC5_051B,
        PropType::Key,
        Value::Key(Key {
            type_id: layout.type_id,
            group: layout.group,
            instance: layout.instance,
        }),
    );
    set_array(
        &mut asset,
        0x0CAB_B2FC,
        PropType::UInt32,
        ids(&panel.config.hover, hide),
    );
    set_array(
        &mut asset,
        0x0CAF_BDC3,
        PropType::UInt32,
        ids(&panel.config.click, hide),
    );
    set_array(
        &mut asset,
        0x0CD4_E49C,
        PropType::UInt32,
        if panel.config.power {
            vec![Value::UInt32(hash("kOnOffToggle"))]
        } else {
            vec![]
        },
    );
    // Suppress the separate native header counter; authored population is a slot.
    set_scalar(
        &mut asset,
        0x0D7E_ADAC,
        PropType::UInt32,
        Value::UInt32(hash("kPopulationNone")),
    );
    let asset_bytes = encode(&asset)?;
    let readback = PropertyFile::parse(&asset_bytes).map_err(|e| e.to_string())?;
    for h in [0x0CAB_B2FC, 0x0CAF_BDC3, 0x0CD4_E49C] {
        let expected = asset.get(h).unwrap();
        if !readback
            .get(h)
            .is_some_and(|p| p.prop_type == expected.prop_type && p.kind == expected.kind)
        {
            return Err(format!("PANEL_PROPERTY_READBACK:{h:08X}"));
        }
    }
    let detail = json!({"schema":panel.path,"fingerprint":panel.fingerprint,"hover":panel.config.hover,"click":panel.config.click,"runtime":{"version":manifest::RUNTIME_VERSION,"tgi":sources::GAME_JS.to_string(),"sha256":sources::digest(&runtime),"output":super::RUNTIME_OUTPUT,"required":true},"layout":layout.to_string()});
    ctx.resources.insert(asset_id, asset_bytes);
    ctx.resources.insert(
        layout,
        serde_json::to_vec(&shell).map_err(|e| e.to_string())?,
    );
    ctx.panel_runtime = Some(runtime);
    Ok(detail)
}
fn has_control(v: &serde_json::Value, id: u32) -> bool {
    v["controlID"].as_u64() == Some(id.into())
        || v["children"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| has_control(v, id)))
}
fn clean_zot(v: &mut serde_json::Value) {
    if v.pointer("/drawable/images")
        .and_then(|a| a.as_array())
        .is_some_and(|a| {
            a.iter().any(|p| {
                p.as_str()
                    .is_some_and(|p| p.ends_with("/rlvr-Zot_Header.png"))
            })
        })
    {
        v["visibility"] = json!(false);
        if let Some(node) = v.as_object_mut() {
            node.remove("drawable");
        }
    }
    if let Some(a) = v["children"].as_array_mut() {
        for c in a {
            clean_zot(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_empty_uses_nonempty_native_selection_sentinel() {
        assert_eq!(
            ids(&[], true),
            vec![Value::UInt32(hash("OpenSCPPanel_empty"))]
        );
    }
    #[test]
    fn feedback_policy_changes_binding_identity() {
        let i = vec!["residence-feedback".into()];
        assert_ne!(ids(&i, true), ids(&i, false));
    }
}
