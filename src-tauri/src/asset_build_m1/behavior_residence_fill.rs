//! EP1 W1 apartment 61EFC000:93E3DB12. The intake and its capacity bins
//! are native references. The ground host supplies construction and access.
use super::behavior_templates::{ENABLE, EXCLUDED, array, contains, key};
use super::property_edit::{set_array_with_flags, set_scalar};
use sc_properties::{Kind, PropType, PropertyFile, Value};

pub(super) const PROFILE: &str = "megatower-residence-v1";
const INTAKE: u32 = 0x78A7_30CD;
const CAPACITY: u32 = 0xEBFC_1753;
const COPAN_INTAKE: u32 = 0x999F_8484;
const UNIT_KIND: u32 = 0x0D02_D846;

pub(super) fn patch(unit: &mut PropertyFile) -> Result<(), String> {
    for hash in ENABLE.into_iter().chain([EXCLUDED]) {
        let original = array(unit, hash)?;
        let mut values = original.to_vec();
        values.retain(|value| {
            ![INTAKE, CAPACITY, COPAN_INTAKE]
                .iter()
                .any(|&id| matches!(value, Value::Key(key) if key.instance == id))
        });
        if hash == ENABLE[3] {
            values.extend([key(INTAKE), key(CAPACITY)]);
        }
        if hash == EXCLUDED {
            values.push(key(COPAN_INTAKE));
        }
        if values != original {
            set_array_with_flags(unit, hash, PropType::Key, values, 0x809C);
        }
    }
    set_scalar(unit, UNIT_KIND, PropType::Key, key(0x31D3_307B));
    if let Some(property) = unit.values.iter_mut().find(|prop| prop.hash == UNIT_KIND) {
        property.encoding.flags = 0x8000;
        property.encoding.array_item_size = None;
    }
    verify(unit)
}

pub(super) fn verify(unit: &PropertyFile) -> Result<(), String> {
    for bundle in [INTAKE, CAPACITY, 0xAB9A_0A3E, 0x9DEF_44FB, 0x933F_0FBF] {
        if !contains(unit, &ENABLE, bundle) || contains(unit, &[EXCLUDED], bundle) {
            return Err(format!("RESIDENCE_FILL_BUNDLE: {bundle:08X}"));
        }
    }
    if contains(unit, &ENABLE, COPAN_INTAKE)
        || !contains(unit, &[EXCLUDED], COPAN_INTAKE)
        || !array(unit, 0x0A88_D179)?.contains(&Value::Int32(22))
        || !unit
            .get(UNIT_KIND)
            .is_some_and(|prop| prop.kind == Kind::Scalar(key(0x31D3_307B)))
    {
        return Err("RESIDENCE_FILL_INTAKE".into());
    }
    Ok(())
}
