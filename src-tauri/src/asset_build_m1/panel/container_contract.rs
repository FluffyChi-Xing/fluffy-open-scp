//! Authored panel contract for a host whose business data lives on modules.
use super::{PanelPlan, PanelSchema, hash, processor::ids, spec::MissingFeedback};
use crate::asset_build_m1::{
    behavior_nodes::ResolvedBehavior, connector::Resources, patch::parse_tgi,
};
use sc_properties::{Key, Kind, PropertyFile, Value};

const TOURIST_CAPACITY: u32 = 0x94D4_5158; // openscpContainerTouristCapacity
const NATIVE_SHOPPER_CAPACITY: u32 = 0x0AD6_A725;
pub(super) const RUNTIME: &str = include_str!(
    "../../../../src/pages/studio/components/asset/panel-editor/native/container-tourists.js"
);

pub(super) fn uses_tourist_capacity(config: &PanelSchema, program: &ResolvedBehavior) -> bool {
    config
        .hover
        .iter()
        .chain(&config.click)
        .any(|item| item == "tourists")
        && program
            .units
            .iter()
            .any(|unit| unit.program.commercial.is_some())
}

pub(super) fn bindings(items: &[String], hide: bool, scoped_tourists: bool, scoped_shoppers: bool, program:Option<&ResolvedBehavior>) -> Vec<Value> {
    let mut result = ids(items, hide);
    for (item, binding) in items.iter().zip(&mut result) {
        if let Some(value)=program.and_then(|p|super::component_queries::binding(item,hide,p)) {
            *binding=value;continue;
        }
        match item.as_str() {
            "tourists" if scoped_tourists => *binding = Value::UInt32(hash("OpenSCPPanel_containerTourists")),
            "shoppers" if scoped_shoppers => *binding = Value::UInt32(hash(super::container_shoppers::BINDING)),
            _ => {}
        }
    }
    result
}

fn tourist_capacity(
    resources: &Resources,
    host: u32,
    program: &ResolvedBehavior,
) -> Result<Value, String> {
    let mut total = 0.0;
    for unit in program
        .units
        .iter()
        .filter(|unit| unit.program.native_tourism.is_some())
    {
        let id = dbpf::ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E0_C000,
            instance: crate::asset_build_m1::behavior_unit_build::instance(host, &unit.id),
        };
        let file = PropertyFile::parse(
            resources
                .get(&id)
                .ok_or("PANEL_CONTAINER_BINDING: missing tourism Unit")?,
        )
        .map_err(|error| error.to_string())?;
        let Some(Value::Float(amount)) = file.get(NATIVE_SHOPPER_CAPACITY).and_then(|p| p.scalar())
        else {
            return Err("PANEL_CONTAINER_BINDING: missing tourism capacity".into());
        };
        if !amount.is_finite() || *amount <= 0.0 {
            return Err("PANEL_CONTAINER_BINDING: invalid tourism capacity".into());
        }
        total += amount;
    }
    if !total.is_finite() || total <= 0.0 {
        return Err("PANEL_CONTAINER_BINDING: invalid total tourism capacity".into());
    }
    Ok(Value::Float(total))
}

pub(super) fn bind_tourist_capacity(
    resources: &Resources,
    host: u32,
    asset: &mut PropertyFile,
    program: &ResolvedBehavior,
) -> Result<(), String> {
    let capacity = tourist_capacity(resources, host, program)?;
    crate::asset_build_m1::property_edit::set_scalar(
        asset,
        TOURIST_CAPACITY,
        sc_properties::PropType::Float,
        capacity,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_every_unsupported_control_with_its_view() {
        let config = PanelSchema {
            schema: "openscp.panel".into(),
            version: 1,
            shell: "67771F5C:0B07405A:2BDCBFDD".into(),
            hover: vec!["workers".into()],
            click: vec!["maintenance".into(), "workers".into(), "density-profit".into()],
            power: true,
            missing_feedback: MissingFeedback::Hide,
        };
        assert_eq!(
            verify_queries(&config, &ResolvedBehavior::default()).unwrap_err(),
            "PANEL_CONTAINER_QUERY_UNSUPPORTED:hover:workers,click:workers,click:density-profit"
        );
    }
}

pub(super) fn verify_queries(
    config: &PanelSchema,
    program: &ResolvedBehavior,
) -> Result<(), String> {
    if !config.power {
        return Err("PANEL_CONTAINER_POWER_REQUIRED".into());
    }
    let has_tourism = program
        .units
        .iter()
        .any(|unit| unit.program.native_tourism.is_some());
    // The native tourist row uses the connected group's shopper bins for its
    // denominator. A retail or hotel component can contribute its own pool.
    let hotel = program
        .units
        .iter()
        .any(|unit| unit.program.hotel.is_some());
    let tourism_shares_retail = program.units.iter().any(|unit| {
        unit.program.native_tourism.is_some() && unit.program.commercial.is_some()
    });
    let retail = program.units.iter().any(|unit| unit.program.commercial.is_some());
    let mut unsupported = Vec::new();
    for (view, items) in [("hover", &config.hover), ("click", &config.click)] {
        for item in items {
            match item.as_str() {
                "maintenance" | "feedback" | "profit" => {}
                "commercial-happiness-reasons" | "commercial-sadness-reasons" if retail && !hotel && !tourism_shares_retail => {}
                "tourists" if has_tourism && !hotel && !tourism_shares_retail => {}
                "shoppers" if retail && !hotel && !tourism_shares_retail => {}
                "hotel-occupancy" if program.hotel.is_some() => {}
                _ if super::component_queries::supported(item,program) => {}
                _ if super::tower_operations::supported(item, program) => {}
                _ => unsupported.push(format!("{view}:{item}")),
            }
        }
    }
    if unsupported.is_empty() {
        Ok(())
    } else {
        Err(format!("PANEL_CONTAINER_QUERY_UNSUPPORTED:{}", unsupported.join(",")))
    }
}

pub(in crate::asset_build_m1) fn verify(
    resources: &Resources,
    host: u32,
    asset: &PropertyFile,
    panel: &PanelPlan,
    program: &ResolvedBehavior,
) -> Result<(), String> {
    verify_queries(&panel.config, program)?;
    super::component_queries::verify(resources, asset, &panel.config, program, host)?;
    let layout = dbpf::ResourceId {
        instance: hash(&format!("OpenSCPPanelShell_{host:08X}")),
        ..parse_tgi(&panel.config.shell)?
    };
    if !resources.contains_key(&layout) {
        return Err("PANEL_CONTAINER_BINDING: missing authored shell".into());
    }
    let hide = panel.config.missing_feedback == MissingFeedback::Hide;
    let scoped_tourists = uses_tourist_capacity(&panel.config, program);
    let scoped_shoppers = super::container_shoppers::enabled(&panel.config, program);
    if scoped_tourists
        && asset.get(TOURIST_CAPACITY).and_then(|p| p.scalar())
            != Some(&tourist_capacity(resources, host, program)?)
    {
        return Err("PANEL_CONTAINER_BINDING: stale tourism capacity".into());
    }
    if scoped_shoppers && asset.get(super::container_shoppers::CAPACITY).and_then(|p| p.scalar())
        != Some(&super::container_shoppers::capacity(resources, host, program)?)
    {
        return Err("PANEL_CONTAINER_BINDING: stale retail capacity".into());
    }
    for (field, expected) in [
        (
            0x0BC5_051B,
            Kind::Scalar(Value::Key(Key {
                type_id: layout.type_id,
                group: layout.group,
                instance: layout.instance,
            })),
        ),
        (
            0x0CAB_B2FC,
            Kind::Array(bindings(&panel.config.hover, hide, scoped_tourists, scoped_shoppers,Some(program))),
        ),
        (
            0x0CAF_BDC3,
            Kind::Array(bindings(&panel.config.click, hide, scoped_tourists, scoped_shoppers,Some(program))),
        ),
        (
            0x0CD4_E49C,
            Kind::Array(vec![Value::UInt32(hash("kOnOffToggle"))]),
        ),
        (
            0x0D7E_ADAC,
            Kind::Scalar(Value::UInt32(hash("OpenSCPPanel_headerNone"))),
        ),
    ] {
        if asset.get(field).map(|property| &property.kind) != Some(&expected) {
            return Err(format!("PANEL_CONTAINER_BINDING:{field:08X}"));
        }
    }
    Ok(())
}
