use dbpf::Package;
use sc_properties::{Kind, PropType, Property, PropertyFile, Value, flatten_parent_inheritance};

use super::{property, property_id};

const UNIT_COST: u32 = 0x0E76_A7E2;
const COST_RESOURCE: u32 = 0x0E76_A7E3;
const RESOURCE_GROUP: u32 = 0x40E0_C100;

pub(super) fn validate(amount: Option<f32>) -> Result<(), String> {
    if amount.is_some_and(|amount| !amount.is_finite() || amount < 0.0) {
        return Err("road_unit_cost_invalid".into());
    }
    Ok(())
}

pub(super) fn apply(
    amount: f32,
    scripts: &Package,
    simulation: &mut PropertyFile,
) -> Result<(), String> {
    if amount > 0.0 {
        // cEcoPathEntry::FillFromProps resolves this key into the budget resource index.
        let donor = flatten_parent_inheritance(simulation.clone(), |key| {
            property(scripts, property_id(key.group, key.instance)).ok()
        });
        let Some(resource) = donor.get(COST_RESOURCE) else {
            return Err("road_unit_cost_resource_missing".into());
        };
        let Some(Value::Key(key)) = resource.scalar() else {
            return Err("road_unit_cost_resource_missing".into());
        };
        if scripts
            .entry(property_id(RESOURCE_GROUP, key.instance))
            .is_none()
        {
            return Err("road_unit_cost_resource_missing".into());
        }
        set(simulation, resource.clone());
    }
    // Both cEcoPathEntry::FillFromProps and GetPaletteRolloverCost read mProps.
    set(
        simulation,
        Property {
            hash: UNIT_COST,
            prop_type: PropType::Float,
            kind: Kind::Scalar(Value::Float(amount)),
            encoding: Default::default(),
        },
    );
    Ok(())
}

fn set(file: &mut PropertyFile, property: Property) {
    if let Some(existing) = file.values.iter_mut().find(|p| p.hash == property.hash) {
        *existing = property;
    } else {
        file.values.push(property);
    }
}
