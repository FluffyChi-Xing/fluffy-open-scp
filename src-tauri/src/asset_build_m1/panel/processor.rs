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
    if config.hover.iter().chain(&config.click).any(|item| item == "hotel-occupancy")
        && plan.behavior_program.as_ref().is_none_or(|program| {
            program.hotel.is_none() && !super::component_queries::supported("hotel-occupancy", program)
        })
    {
        return Err("PANEL_BEHAVIOR_REQUIRED:hotel-occupancy:hotel".into());
    }
    if let Some(program) = plan.behavior_program.as_ref()
        .filter(|program| !program.units.is_empty() && !program.office_and_retail)
    {
        super::container_contract::verify_queries(&config, program)?;
    }
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
    let packages = sources::open(root, &dependency.sources)?;
    let tower_operations = super::tower_operations::selected(&config);
    if tower_operations {
        if plan.behavior_program.as_ref().is_none_or(|program| {
            config.hover.iter().chain(&config.click).filter(|id| {
                components().iter().any(|component| component.id == **id && component.tower_operations)
            }).any(|id| !super::tower_operations::supported(id, program))
        }) {
            return Err("PANEL_TOWER_OPERATIONS_REQUIRE_COMPONENTS".into());
        }
        super::tower_operations::verify_layouts(&config, &packages)?;
    }
    let mut fingerprint_bytes = serde_json::to_vec(&(
        &config,
        &dependency,
        super::RUNTIME,
        super::MESSAGES,
        super::layout::COMPILER_VERSION,
        components(),
    ))
    .map_err(|e| e.to_string())?;
    fingerprint_bytes.extend_from_slice(super::container_contract::RUNTIME.as_bytes());
    fingerprint_bytes.extend_from_slice(super::container_shoppers::RUNTIME.as_bytes());
    fingerprint_bytes.extend_from_slice(super::tower_operations::RUNTIME.as_bytes());
    fingerprint_bytes.extend_from_slice(super::component_queries::RUNTIME.as_bytes());
    fingerprint_bytes.extend_from_slice(super::runtime_bundle::COMPILER_VERSION.as_bytes());
    fingerprint_bytes.extend_from_slice(b"shared-runtime-export-v1");
    let fingerprint = sources::digest(&fingerprint_bytes);
    plan.panel = Some(PanelPlan {
        path: dependency.schema.clone(),
        config,
        dependency,
        fingerprint,
    });
    Ok(())
}

pub(super) fn ids(items: &[String], hide: bool) -> Vec<Value> {
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
    super::layout::prepare_shell(&mut shell)?;
    let layout = ResourceId {
        instance: hash(&format!("OpenSCPPanelShell_{instance:08X}")),
        ..source_id
    };
    if ctx.resources.contains_key(&layout) {
        return Err("PANEL_LAYOUT_ID_COLLISION".into());
    }
    let source_game = sources::read(&packages, sources::GAME_JS)?;
    super::validate_stock_bindings(&source_game)?;
    let tower_operations = super::tower_operations::selected(&panel.config);
    if ctx
        .resources
        .get(&sources::GAME_JS)
        .is_some_and(|old| old != &source_game)
    {
        return Err("PANEL_RUNTIME_OVERLAY_CONFLICT".into());
    }
    ctx.resources.remove(&sources::GAME_JS);
    let asset_id = ResourceId {
        type_id: 0x00B1_B104,
        group: 0x40E1_C000,
        instance,
    };
    let mut asset = PropertyFile::parse(ctx.resources.get(&asset_id).ok_or("PANEL_ASSET_MISSING")?)
        .map_err(|e| e.to_string())?;
    if tower_operations && ctx.plan.behavior_program.as_ref().is_some_and(|program| program.office_and_retail) {
        set_scalar(&mut asset, super::tower_operations::SINGLE_BUSINESS, PropType::Float, Value::Float(1.0));
    }
    let scoped_tourists = ctx.plan.behavior_program.as_ref().filter(|program| {
        super::container_contract::uses_tourist_capacity(&panel.config, program)
    });
    if let Some(program) = scoped_tourists {
        super::container_contract::bind_tourist_capacity(&ctx.resources, instance, &mut asset, program)?;
    }
    let scoped_shoppers = ctx.plan.behavior_program.as_ref().filter(|program| {
        super::container_shoppers::enabled(&panel.config, program)
    });
    // The game loads one UI overlay. The newer shoppers runtime also supports
    // previously built containers that use the scoped tourist binding.
    if let Some(program) = scoped_shoppers {
        super::container_shoppers::write(&ctx.resources, instance, &mut asset, program)?;
    }
    if let Some(program) = ctx.plan.behavior_program.as_ref() {
        super::component_queries::write(&mut asset,&panel.config,program,instance)?;
    }
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
        super::container_contract::bindings(&panel.config.hover, hide, scoped_tourists.is_some(), scoped_shoppers.is_some(),ctx.plan.behavior_program.as_ref()),
    );
    set_array(
        &mut asset,
        0x0CAF_BDC3,
        PropType::UInt32,
        super::container_contract::bindings(&panel.config.click, hide, scoped_tourists.is_some(), scoped_shoppers.is_some(),ctx.plan.behavior_program.as_ref()),
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
        Value::UInt32(hash("OpenSCPPanel_headerNone")),
    );
    let ledger = if panel
        .config
        .hover
        .iter()
        .chain(&panel.config.click)
        .any(|id| matches!(id.as_str(), "profit" | "tourism-profit"))
    {
        let unit_id = ResourceId {
            group: 0x40E0_C000,
            ..asset_id
        };
        let mut unit =
            PropertyFile::parse(ctx.resources.get(&unit_id).ok_or("PANEL_UNIT_MISSING")?)
                .map_err(|e| e.to_string())?;
        let mut effective = unit.clone();
        let mut cursor = unit.clone();
        let mut seen = std::collections::HashSet::new();
        while let Some(Value::Key(parent)) = cursor.get(0x00B2_CCCB).and_then(|p| p.scalar()) {
            let id = ResourceId {
                type_id: parent.type_id,
                group: parent.group,
                instance: parent.instance,
            };
            if !seen.insert(id) {
                return Err("PANEL_UNIT_PARENT_CYCLE".into());
            }
            let bytes = if let Some(bytes) = ctx.resources.get(&id) {
                bytes.clone()
            } else {
                sources::read(&packages, id)?
            };
            cursor = PropertyFile::parse(&bytes).map_err(|e| e.to_string())?;
            for p in &cursor.values {
                if effective.get(p.hash).is_none() {
                    effective.values.push(p.clone());
                }
            }
        }
        let provider_id = ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E0_C700,
            instance: super::ledger::PROVIDER,
        };
        let native = PropertyFile::parse(&sources::read(&packages, provider_id)?)
            .map_err(|e| e.to_string())?;
        let added = super::ledger::ensure(&mut unit, &effective, &native)?;
        if added {
            let bytes = encode(&unit)?;
            let readback = PropertyFile::parse(&bytes).map_err(|e| e.to_string())?;
            let mut verify = readback.clone();
            if super::ledger::ensure(&mut verify, &readback, &native)? {
                return Err("PANEL_PROFIT_LEDGER_READBACK".into());
            }
            ctx.resources.insert(unit_id, bytes);
        }
        json!({"provider":provider_id.to_string(),"added":added})
    } else {
        serde_json::Value::Null
    };
    let asset_bytes = encode(&asset)?;
    let readback = PropertyFile::parse(&asset_bytes).map_err(|e| e.to_string())?;
    for h in [
        0x0BC5_051B,
        0x0CAB_B2FC,
        0x0CAF_BDC3,
        0x0CD4_E49C,
        0x0D7E_ADAC,
    ] {
        let expected = asset.get(h).unwrap();
        if !readback
            .get(h)
            .is_some_and(|p| p.prop_type == expected.prop_type && p.kind == expected.kind)
        {
            return Err(format!("PANEL_PROPERTY_READBACK:{h:08X}"));
        }
    }
    let detail = json!({"schema":panel.path,"fingerprint":panel.fingerprint,"hover":panel.config.hover,"click":panel.config.click,"ledger":ledger,"runtime":{"version":manifest::RUNTIME_VERSION,"tgi":sources::GAME_JS.to_string(),"shared":true,"required":true},"layout":layout.to_string()});
    ctx.resources.insert(asset_id, asset_bytes);
    ctx.resources.insert(
        layout,
        serde_json::to_vec(&shell).map_err(|e| e.to_string())?,
    );
    Ok(detail)
}
fn has_control(v: &serde_json::Value, id: u32) -> bool {
    v["controlID"].as_u64() == Some(id.into())
        || v["children"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| has_control(v, id)))
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
