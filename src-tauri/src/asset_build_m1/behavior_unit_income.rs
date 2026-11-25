//! EP1 DD6E83EC specializes income bin 0F20C597 to twice the floor
//! maintenance field via cEcoUnitEntry::AddBundlePropOverrides (00644F20).
//! Isolated units keep the same specialization but read a separate Unit field.
use super::{
    behavior_nodes::ResolvedBehavior,
    behavior_templates::{ENABLE, EXCLUDED, array, contains, key},
    behavior_unit_build::instance,
    connector::Resources,
    property_edit::{encode, set_array_with_flags, set_scalar},
};
use dbpf::ResourceId;
use sc_properties::{Key, PropType, PropertyFile, Value};

const GROUP: u32 = 0x40E0_C700;
const FIELD: u32 = 0xA00F_E305;
const MAINTENANCE: u32 = 0x09AE_19D7;
const INCOME_BIN: u32 = 0x0F20_C597;

pub(super) fn capacity_input(program: &ResolvedBehavior) -> Option<f32> {
    if program.office.is_some() {
        // Medium-tech office 61EFC000:6EF67CC5.
        Some(1460.0)
    } else if program
        .commercial
        .as_ref()
        .is_some_and(|recipe| recipe.uses_tower_income())
    {
        // Standard W2 mall 61EFC000:8A807C96, also the mixed mall economy donor.
        Some(1150.0)
    } else {
        None
    }
}

pub(super) fn bundle_id(child: u32) -> ResourceId {
    ResourceId {
        type_id: super::behavior::PROPERTY_TYPE,
        group: GROUP,
        instance: instance(child, "tower-income-capacity"),
    }
}

fn bundle_key(id: ResourceId) -> Value {
    Value::Key(Key {
        type_id: id.type_id,
        group: id.group,
        instance: id.instance,
    })
}

fn bundle() -> Result<Vec<u8>, String> {
    let mut file = PropertyFile::default();
    for (hash, ty, value) in [
        (0x0A6F_E749, PropType::Key, key(INCOME_BIN)),
        (0x0A6F_E74A, PropType::UInt32, Value::UInt32(FIELD)),
        (0x0A6F_E74D, PropType::Float, Value::Float(2.0)),
    ] {
        set_array_with_flags(&mut file, hash, ty, vec![value], 0x809C);
    }
    encode(&file)
}

pub(super) fn bind(
    resources: &Resources,
    unit_id: ResourceId,
    input: f32,
) -> Result<(Vec<u8>, ResourceId, Vec<u8>), String> {
    let id = bundle_id(unit_id.instance);
    let bytes = resources
        .get(&unit_id)
        .ok_or("BEHAVIOR_UNIT_INCOME: missing Unit")?;
    let mut unit = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
    if id.instance == 0
        || resources.contains_key(&id)
        || contains(&unit, &ENABLE, id.instance)
        || contains(&unit, &[EXCLUDED], id.instance)
        || unit.get(FIELD).is_some()
    {
        return Err("BEHAVIOR_UNIT_INCOME: specialization identity collision".into());
    }
    let mut keys = array(&unit, ENABLE[3])?.to_vec();
    // The mixed mall shopper override has a separate final-position contract.
    let shopper_id = super::behavior_commercial_mixed::bundle_id(unit_id);
    let index = keys
        .iter()
        .position(|key| *key == bundle_key(shopper_id))
        .unwrap_or(keys.len());
    keys.insert(index, bundle_key(id));
    set_array_with_flags(&mut unit, ENABLE[3], PropType::Key, keys, 0x809C);
    set_scalar(&mut unit, FIELD, PropType::Float, Value::Float(input));
    Ok((encode(&unit)?, id, bundle()?))
}

pub(super) fn verify(
    resources: &Resources,
    unit: &PropertyFile,
    child: u32,
    input: f32,
) -> Result<(), String> {
    let id = bundle_id(child);
    let keys = array(unit, ENABLE[3])?;
    let native = keys.iter().position(|key| {
        matches!(key, Value::Key(key) if key.instance == 0xDD6E_83EC && key.group == 0x61EF_C700)
    });
    let override_key = bundle_key(id);
    if !native.is_some_and(|index| keys[index + 1..].contains(&override_key))
        || contains(unit, &[EXCLUDED], id.instance)
        || unit.get(FIELD).and_then(|prop| prop.scalar()) != Some(&Value::Float(input))
        || unit.get(MAINTENANCE).and_then(|prop| prop.scalar()) != Some(&Value::Float(0.0))
        || resources.get(&id) != Some(&bundle()?)
    {
        return Err(
            "BEHAVIOR_UNIT_INCOME: missing or invalid income capacity specialization".into(),
        );
    }
    Ok(())
}
