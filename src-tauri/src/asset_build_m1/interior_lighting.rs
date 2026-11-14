//! Default controls and written-package checks for native interior lighting.

use dbpf::ResourceId;
use sc_properties::{Key, Kind, PropType, PropertyFile, Value};

use super::connector::Resources;
use super::menu::{GRAPHICS_UNIT_GROUP, HASH_PARENT};
use super::property_edit::set_scalar;

pub(super) const ACTIVE_CONTROLLER: u32 = 0x0F16_9C46;
const LIGHTS_WHEN_ACTIVE: u32 = 0x0E0B_E186;

pub(super) fn write_graphics_defaults(graphics: &mut PropertyFile) {
    for hash in [LIGHTS_WHEN_ACTIVE, ACTIVE_CONTROLLER] {
        set_scalar(graphics, hash, PropType::Bool, Value::Bool(true));
    }
}

pub(super) fn verify_graphics_switches(bytes: &[u8]) -> Result<PropertyFile, String> {
    let graphics = PropertyFile::parse(bytes)
        .map_err(|error| format!("graphics unit lighting contract: {error}"))?;
    for hash in [ACTIVE_CONTROLLER, LIGHTS_WHEN_ACTIVE] {
        require_switch(&graphics, hash, "Graphics Unit")?;
    }
    Ok(graphics)
}

pub(super) fn verify_written(
    resources: &Resources,
    lot_id: ResourceId,
    lot: &PropertyFile,
) -> Result<(), String> {
    // Asset Editor's Packager backend_compile.py:206 sets this on the Lot.
    require_switch(lot, ACTIVE_CONTROLLER, "Lot")?;
    let graphics_id = ResourceId { group: GRAPHICS_UNIT_GROUP, ..lot_id };
    let bytes = resources.get(&graphics_id)
        .ok_or_else(|| format!("written lighting contract: Graphics Unit {graphics_id} is missing"))?;
    let graphics = verify_graphics_switches(bytes)?;
    let expected_parent = Value::Key(Key {
        type_id: lot_id.type_id,
        group: lot_id.group,
        instance: lot_id.instance,
    });
    if graphics.get(HASH_PARENT).and_then(|property| property.scalar()) != Some(&expected_parent) {
        return Err(format!("written lighting contract: Graphics Unit {graphics_id} must inherit Lot {lot_id}"));
    }
    Ok(())
}

/// The native landmark bundle supplies F1FD62EF, which sets ActiveBuilding.
/// The graphics switches alone cannot supply that simulation state.
pub(super) fn verify_landmark_activation(unit: &PropertyFile) -> Result<(), String> {
    const BUILDING_TYPE: u32 = 0x406C_CC43;
    let has_building_type = |hash| {
        matches!(unit.get(hash).map(|property| &property.kind), Some(Kind::Array(values))
            if values.iter().any(|value| matches!(value, Value::Key(key) if key.instance == BUILDING_TYPE)))
    };
    if !(0x0A02_09A2..=0x0A02_09A5).any(has_building_type) || has_building_type(0x0A02_09B2) {
        return Err("landmark lighting contract: ActiveBuilding bundle 406CCC43 must be enabled".into());
    }
    Ok(())
}

fn require_switch(file: &PropertyFile, hash: u32, resource: &str) -> Result<(), String> {
    if file.get(hash).and_then(|property| property.scalar()) != Some(&Value::Bool(true)) {
        return Err(format!("lighting contract: {resource} {hash:08X} must be Bool(true)"));
    }
    Ok(())
}
