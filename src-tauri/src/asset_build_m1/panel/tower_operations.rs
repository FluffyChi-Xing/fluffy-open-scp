//! Local DLC layouts with queries adapted to an ordinary building's modules.
use super::{sources, PanelSchema};
use crate::asset_build_m1::behavior_nodes::ResolvedBehavior;
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};

pub(super) const SINGLE_BUSINESS: u32 = 0x739B_3A2F;

pub(in crate::asset_build_m1) fn verify_single_business(panel: &super::PanelPlan, asset: &PropertyFile) -> Result<(), String> {
    if !selected(&panel.config) {
        return Ok(());
    }
    if asset.get(SINGLE_BUSINESS).and_then(|property| property.scalar()) != Some(&Value::Float(1.0)) {
        return Err("PANEL_TOWER_OPERATIONS_REQUIRE_COMPONENTS".into());
    }
    let hide = panel.config.missing_feedback == super::spec::MissingFeedback::Hide;
    for (hash, items) in [(0x0CAB_B2FC, &panel.config.hover), (0x0CAF_BDC3, &panel.config.click)] {
        if asset.get(hash).map(|property| &property.kind) != Some(&Kind::Array(super::processor::ids(items, hide))) {
            return Err(format!("PANEL_CONTAINER_BINDING: readback differs at {hash:08X}"));
        }
    }
    Ok(())
}
pub(super) const RUNTIME: &str = include_str!(
    "../../../../src/pages/studio/components/asset/panel-editor/native/tower-operations.js"
);
const LAYOUTS: [(&str, u32); 3] = [
    ("workers-by-wealth", 0xA17E_4260),
    ("mall-average-shoppers", 0x7E29_FFC1),
    ("tower-business-profit", 0x4E84_E360),
];

pub(super) fn selected(config: &PanelSchema) -> bool {
    config.hover.iter().chain(&config.click).any(|id| LAYOUTS.iter().any(|(control, _)| control == id))
}

pub(super) fn verify_layouts(config: &PanelSchema, packages: &[Package]) -> Result<(), String> {
    for (control, instance) in LAYOUTS {
        if !config.hover.iter().chain(&config.click).any(|id| id == control) { continue; }
        let id = ResourceId { type_id: 0x6777_1F5C, group: 0x0B07_4E5A, instance };
        if sources::read(packages, id).is_err() {
            return Err(format!("PANEL_TOWER_LAYOUT_MISSING:{control}:{id}"));
        }
    }
    Ok(())
}

pub(super) fn supported(control: &str, program: &ResolvedBehavior) -> bool {
    if program.office_and_retail {
        return LAYOUTS.iter().any(|(id, _)| *id == control);
    }
    let office = program.units.iter().any(|unit| unit.program.office.is_some() || unit.program.office_and_retail);
    let mall = program.units.iter().any(|unit| unit.program.commercial.as_ref()
        .is_some_and(|recipe| recipe.bundles().contains(&0xF51A_B185)));
    let unrelated_ledger = program.units.iter().any(|unit| unit.program.native_tourism.is_some() || unit.program.hotel.is_some());
    match control {
        "workers-by-wealth" => office,
        "mall-average-shoppers" => mall,
        "tower-business-profit" => office && mall && !unrelated_ledger,
        _ => false,
    }
}
