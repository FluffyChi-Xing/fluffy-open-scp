//! Live shopper counts from connected Units and capacity from retail Units only.
use super::PanelSchema;
use crate::asset_build_m1::{behavior_nodes::ResolvedBehavior, connector::Resources, property_edit::set_scalar};
use sc_properties::{PropType, PropertyFile, Value};

pub(super) const CAPACITY: u32 = 0xA6A4_30EF;
pub(super) const BINDING: &str = "OpenSCPPanel_containerShoppers";
pub(super) const RUNTIME: &str = include_str!(
    "../../../../src/pages/studio/components/asset/panel-editor/native/container-shoppers.js"
);

pub(super) fn enabled(config: &PanelSchema, program: &ResolvedBehavior) -> bool {
    config.hover.iter().chain(&config.click).any(|id| id == "shoppers")
        && program.units.iter().any(|scope| scope.program.commercial.is_some())
}

pub(super) fn capacity(resources: &Resources, host: u32, program: &ResolvedBehavior) -> Result<Value, String> {
    let mut total = 0.0;
    for scope in program.units.iter().filter(|scope| scope.program.commercial.is_some()) {
        let id = dbpf::ResourceId {
            type_id: 0x00B1_B104, group: 0x40E0_C000,
            instance: crate::asset_build_m1::behavior_unit_build::instance(host, &scope.id),
        };
        let bytes = resources.get(&id).ok_or_else(|| format!("PANEL_CONTAINER_BINDING: missing retail Unit {id}"))?;
        let unit = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
        let amount = match unit.get(0x0AD6_A725).and_then(|property| property.scalar()) {
            Some(Value::Float(amount)) if amount.is_finite() && *amount > 0.0 => *amount,
            _ => return Err(format!("PANEL_CONTAINER_BINDING: invalid retail capacity on {id}")),
        };
        total += amount;
    }
    if !total.is_finite() || total <= 0.0 {
        return Err("PANEL_CONTAINER_BINDING: invalid total retail capacity".into());
    }
    Ok(Value::Float(total))
}

pub(super) fn write(resources: &Resources, host: u32, asset: &mut PropertyFile, program: &ResolvedBehavior) -> Result<(), String> {
    let value = capacity(resources, host, program)?;
    set_scalar(asset, CAPACITY, PropType::Float, value);
    Ok(())
}
