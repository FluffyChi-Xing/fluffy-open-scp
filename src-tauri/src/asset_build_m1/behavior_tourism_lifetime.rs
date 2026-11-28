//! Missing native visitor transport specialization, separate from proven recipes.
//! Tokyo Tower 40E0C000:084DB48F owns 0D9114D4 = Vector2(120,120).
//! Base AEB 64E8EB50/51/52 specialize transport slot 1 using op 26;
//! retail engine SpecializeRule assigns this Vector2 to TransportOp::mLife.

use super::property_edit::encode;
use sc_properties::{Kind, PropType, PropertyFile, Value};

const LIFETIME: u32 = 0x0D91_14D4;
const NATIVE: &[u8] = include_bytes!("../../resources/sc5/tourism-unit-anchors/084db48f.bin");

fn verify(unit: &PropertyFile) -> Result<(), String> {
    let property = unit
        .get(LIFETIME)
        .ok_or("TOURISM_TRANSPORT_LIFETIME: missing 0D9114D4")?;
    if property.prop_type != PropType::Vector2
        || !matches!(&property.kind, Kind::Scalar(Value::Vector2(value))
            if value.iter().all(|v| v.is_finite() && *v > 0.0))
    {
        return Err("TOURISM_TRANSPORT_LIFETIME: expected positive finite Vector2".into());
    }
    Ok(())
}

pub(super) fn patch(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut unit = PropertyFile::parse(bytes)
        .map_err(|error| format!("TOURISM_TRANSPORT_LIFETIME: {error}"))?;
    if unit.get(LIFETIME).is_some() {
        verify(&unit)?;
        return Ok(bytes.to_vec());
    }
    // Copy the whole native property, including encoding; never invent flags.
    let native = PropertyFile::parse(NATIVE)
        .map_err(|error| format!("TOURISM_TRANSPORT_LIFETIME: native anchor: {error}"))?;
    verify(&native)?;
    unit.values.push(native.get(LIFETIME).unwrap().clone());
    let result = encode(&unit)?;
    let written = PropertyFile::parse(&result)
        .map_err(|error| format!("TOURISM_TRANSPORT_LIFETIME: readback: {error}"))?;
    verify(&written)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::super::property_edit::set_scalar;
    use super::*;

    #[test]
    fn restores_exact_native_property_and_preserves_unrelated_fields() {
        let mut unit = PropertyFile::parse(NATIVE).unwrap();
        let original = unit.get(LIFETIME).unwrap().clone();
        assert_eq!(original.kind, Kind::Scalar(Value::Vector2([120.0, 120.0])));
        unit.values.retain(|p| p.hash != LIFETIME);
        let before = unit.values.clone();
        let result = patch(&encode(&unit).unwrap()).unwrap();
        let after = PropertyFile::parse(&result).unwrap();
        assert_eq!(after.get(LIFETIME).unwrap(), &original);
        let unrelated: Vec<_> = after
            .values
            .iter()
            .filter(|p| p.hash != LIFETIME)
            .cloned()
            .collect();
        assert_eq!(unrelated, before);
        assert_eq!(patch(&result).unwrap(), result);
    }

    #[test]
    fn keeps_authored_lifetime_bytes_and_rejects_invalid_specialization() {
        let mut unit = PropertyFile::parse(NATIVE).unwrap();
        for value in [
            Value::Vector2([90.0, 180.0]),
            Value::Float(120.0),
            Value::Vector2([0.0, 120.0]),
            Value::Vector2([f32::NAN, 120.0]),
        ] {
            let valid = value == Value::Vector2([90.0, 180.0]);
            let ty = if matches!(value, Value::Float(_)) {
                PropType::Float
            } else {
                PropType::Vector2
            };
            set_scalar(&mut unit, LIFETIME, ty, value);
            let bytes = encode(&unit).unwrap();
            if valid {
                assert_eq!(patch(&bytes).unwrap(), bytes);
            } else {
                assert!(
                    patch(&bytes)
                        .unwrap_err()
                        .starts_with("TOURISM_TRANSPORT_LIFETIME:")
                );
            }
        }
    }
}
