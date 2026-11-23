//! Native container presentation. All selected rows query connected units;
//! the host has no population, opening hours or business income properties.
//! Native source: game-ui.js kMaintenance / kBlockLandmarkTourists /
//! kPopulationNone; GB::GetUnitBoundingBox -> GetUnitBoundingBoxInternal;
//! SC::cBarGraphManager::Update uses each unit's graphics bounding box.
use super::{behavior_nodes::ResolvedBehavior, connector::Resources, menu,
    property_edit::{encode, set_array, set_scalar}};
use dbpf::ResourceId;
use sc_properties::{Kind, PropType, PropertyFile, Value};

const HOVER: u32 = 0x0CAB_B2FC;
const CLICK: u32 = 0x0CAF_BDC3;
const HEADER: u32 = 0x0D7E_ADAC;
pub(super) const BOUNDS: u32 = 0x00F9_EFBA;
fn id(group: u32, instance: u32) -> ResourceId {
    ResourceId {type_id: menu::PROPERTY_TYPE, group, instance}
}
fn rows(program: &ResolvedBehavior) -> Vec<Value> {
    let mut names = vec!["kMaintenance"];
    if program.units.iter().any(|u| u.program.native_tourism.is_some()) {
        names.push("kBlockLandmarkTourists");
    }
    names.into_iter().map(|n| Value::UInt32(super::panel::hash(n))).collect()
}
pub(super) fn patch(resources: &mut super::package::BuildResources, host: u32,
    program: &ResolvedBehavior) -> Result<(), String> {
    let asset_id = id(menu::lot_group(), host);
    let mut asset = PropertyFile::parse(resources.get(&asset_id)
        .ok_or("CONTAINER_UI: missing host asset")?).map_err(|e|e.to_string())?;
    for hash in [HOVER, CLICK] { set_array(&mut asset, hash, PropType::UInt32, rows(program)); }
    set_scalar(&mut asset, HEADER, PropType::UInt32,
        Value::UInt32(super::panel::hash("kPopulationNone")));
    set_array(&mut asset, 0x0CD4_E49C, PropType::UInt32,
        vec![Value::UInt32(super::panel::hash("kOnOffToggle"))]);
    resources.insert(asset_id, encode(&asset)?);
    // The single-business tooltip parameter belongs to a component, not the
    // container. Do not show a fabricated aggregate from the host's default.
    let menu_id = id(menu::MENU_GROUP, host);
    let mut menu = PropertyFile::parse(resources.get(&menu_id)
        .ok_or("CONTAINER_UI: missing menu")?).map_err(|e|e.to_string())?;
    menu.values.retain(|p| ![0x0EB1_FC05, 0x0EB1_FC22].contains(&p.hash));
    resources.insert(menu_id, encode(&menu)?);
    Ok(())
}
pub(super) fn verify(resources: &Resources, host: u32,
    program: &ResolvedBehavior) -> Result<(), String> {
    verify_presentation(resources, host, program, None)
}

pub(super) fn verify_presentation(resources: &Resources, host: u32,
    program: &ResolvedBehavior, panel: Option<&super::panel::PanelPlan>) -> Result<(), String> {
    let asset = PropertyFile::parse(resources.get(&id(menu::lot_group(),host))
        .ok_or("CONTAINER_UI: missing host asset")?).map_err(|e|e.to_string())?;
    if let Some(panel) = panel {
        super::panel::container_contract::verify(resources, host, &asset, panel, program)?;
    } else {
        let toggle = Value::UInt32(super::panel::hash("kOnOffToggle"));
        if asset.get(0x0CD4_E49C).map(|p| &p.kind) != Some(&Kind::Array(vec![toggle])) {
            return Err("CONTAINER_UI: expected exactly one whole-building toggle".into());
        }
        for hash in [HOVER, CLICK] {
            if asset.get(hash).map(|p|&p.kind) != Some(&Kind::Array(rows(program))) {
                return Err("CONTAINER_UI: host-only or unbound rollover query".into());
            }
        }
        if asset.get(HEADER).and_then(|p|p.scalar()) !=
            Some(&Value::UInt32(super::panel::hash("kPopulationNone"))) {
            return Err("CONTAINER_UI: business-free host has a population counter".into());
        }
    }
    let menu = PropertyFile::parse(resources.get(&id(menu::MENU_GROUP,host))
        .ok_or("CONTAINER_UI: missing menu")?).map_err(|e|e.to_string())?;
    for hash in [0x0EB1_FC05, 0x0EB1_FC22] {
        if menu.get(hash).is_some() {
            return Err("CONTAINER_UI: tooltip references a component-only parameter".into());
        }
    }
    Ok(())
}

/// Copy the native render bounds, not the model. The previous model-free
/// module inherited a tiny default box, producing misplaced narrow bars.
pub(super) fn copy_bounds(host: &PropertyFile, child: &mut PropertyFile) {
    if let Some(bounds) = host.get(BOUNDS) {
        child.values.retain(|p|p.hash != BOUNDS);
        child.values.push(bounds.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replaces_host_business_queries_and_rejects_old_population_header() {
        let program = super::super::behavior_nodes::compile(None, &[
            serde_json::json!({"id":"tourism","type":"tourism","unit":"visits",
                "params":{"tourismProfile":"native-tourism-v1"}}),
        ]).unwrap();
        let host = 42;
        let empty = encode(&PropertyFile::default()).unwrap();
        let mut resources = super::super::package::BuildResources::from_source(
            [(id(menu::lot_group(),host), empty.clone()),
             (id(menu::MENU_GROUP,host),empty)].into_iter().collect());
        patch(&mut resources,host,&program).unwrap();
        verify(resources.all(),host,&program).unwrap();
        let mut asset = PropertyFile::parse(resources.get(&id(menu::lot_group(),host)).unwrap()).unwrap();
        assert_eq!(asset.get(CLICK).unwrap().kind,Kind::Array(vec![
            Value::UInt32(super::super::panel::hash("kMaintenance")),
            Value::UInt32(super::super::panel::hash("kBlockLandmarkTourists")),
        ]));
        let toggle = Value::UInt32(super::super::panel::hash("kOnOffToggle"));
        let mut duplicate = asset.clone();
        set_array(&mut duplicate, 0x0CD4_E49C, PropType::UInt32, vec![toggle.clone(), toggle]);
        resources.insert(id(menu::lot_group(),host), encode(&duplicate).unwrap());
        assert!(verify(resources.all(), host, &program).unwrap_err().contains("exactly one"));
        set_scalar(&mut asset, HEADER, PropType::UInt32,
            Value::UInt32(super::super::panel::hash("kPopulationResidents")));
        resources.insert(id(menu::lot_group(),host),encode(&asset).unwrap());
        assert!(verify(resources.all(),host,&program).is_err());
    }
    #[test]
    fn native_bounds_copy_preserves_encoding_and_adds_no_model() {
        let bytes = include_bytes!("../../resources/sc5/tourism-unit-anchors/084db48f.bin");
        let mut host = PropertyFile::parse(bytes).unwrap();
        set_scalar(&mut host, BOUNDS, PropType::BoundingBox,
            Value::BoundingBox{min:[-20.0,-30.0,0.0],max:[20.0,30.0,180.0]});
        let host = PropertyFile::parse(&encode(&host).unwrap()).unwrap();
        let mut child = PropertyFile::default();
        copy_bounds(&host,&mut child);
        let decoded = PropertyFile::parse(&encode(&child).unwrap()).unwrap();
        assert_eq!(decoded.get(BOUNDS),host.get(BOUNDS));
        assert_eq!(decoded.values.len(),1);
    }
}
