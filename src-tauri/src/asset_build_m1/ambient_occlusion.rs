//! Directional AO from SC5 Asset Packager (2), ambient_occlusion.py:5–36.
//! The verified bytecode writes two scalar floats to the render Asset.

use sc_properties::{PropType, PropertyFile, Value};
use serde::Serialize;
use serde_json::Value as Json;

const STRENGTH: u32 = 0x0DA7_6A05;
const CURVE: u32 = 0x0DA7_6A06;
const FLAGS: u16 = 0x8000;

#[derive(Debug, Serialize)]
pub(super) struct Settings {
    profile: &'static str,
    strength: f32,
    curve: f32,
}

impl Settings {
    pub(super) fn parse(raw: &Json) -> Result<Self, String> {
        let settings = raw
            .as_object()
            .ok_or("semantics.ambientOcclusion must be an object")?;
        let profile = settings.get("aoProfile").map_or(Some("E"), Json::as_str);
        let (profile, strength, curve) = match profile {
            Some("E") => ("E", 0.45, 2.5),
            Some("F") => ("F", 0.7, 1.2),
            Some("custom") => {
                let number = |key, default, bounds: std::ops::RangeInclusive<f64>| {
                    settings
                        .get(key)
                        .map_or(Some(default), Json::as_f64)
                        .filter(|value| bounds.contains(value))
                        .ok_or_else(|| {
                            format!(
                                "semantics.ambientOcclusion.{key} must be a number in {bounds:?}"
                            )
                        })
                };
                (
                    "custom",
                    number("aoStrength", 0.45, 0.0..=1.0)?,
                    number("aoCurve", 2.5, 0.1..=8.0)?,
                )
            }
            _ => return Err("semantics.ambientOcclusion.aoProfile must be E, F, or custom".into()),
        };
        Ok(Self {
            profile,
            strength: strength as f32,
            curve: curve as f32,
        })
    }

    pub(super) fn apply(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let mut asset = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
        for (hash, value) in [(STRENGTH, self.strength), (CURVE, self.curve)] {
            // Author encoding: struct.pack('>IHHf', hash, 13, 32768, value).
            asset.values.retain(|property| property.hash != hash);
            super::property_edit::set_scalar(
                &mut asset,
                hash,
                PropType::Float,
                Value::Float(value),
            );
            asset
                .values
                .last_mut()
                .expect("new AO field")
                .encoding
                .flags = FLAGS;
        }
        super::property_edit::encode(&asset)
    }

    pub(super) fn verify(&self, bytes: &[u8]) -> Result<(), String> {
        let asset = PropertyFile::parse(bytes).map_err(|error| error.to_string())?;
        for (hash, value) in [(STRENGTH, self.strength), (CURVE, self.curve)] {
            let valid = asset.get(hash).is_some_and(|property| {
                property.prop_type == PropType::Float && property.encoding.flags == FLAGS &&
                    matches!(property.scalar(), Some(Value::Float(actual)) if actual.to_bits() == value.to_bits())
            });
            if !valid {
                return Err(format!(
                    "ambient occlusion: written Asset property {hash:08X} must be Float({value}) with flags {FLAGS:04X}"
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn writes_author_float_records_and_checks_the_written_asset() {
        let e = Settings::parse(&json!({})).unwrap();
        let f = Settings::parse(&json!({"aoProfile":"F"})).unwrap();
        let original = [0; 4];
        let encoded = e.apply(&original).unwrap();
        assert_eq!(
            encoded,
            [
                0, 0, 0, 2, 0x0d, 0xa7, 0x6a, 5, 0, 13, 0x80, 0, 0x3e, 0xe6, 0x66, 0x66, 0x0d,
                0xa7, 0x6a, 6, 0, 13, 0x80, 0, 0x40, 0x20, 0, 0,
            ]
        );
        e.verify(&encoded).unwrap();
        assert!(f.verify(&encoded).is_err());
        let updated = f.apply(&encoded).unwrap();
        f.verify(&updated).unwrap();
        assert!(f.verify(&original).is_err());
        assert!(Settings::parse(&json!({"aoProfile":"custom","aoStrength":true})).is_err());
        assert!(Settings::parse(&json!({"aoProfile":"custom","aoCurve":9})).is_err());
        assert!(Settings::parse(&json!({"aoProfile":"custom","aoStrength":0,"aoCurve":8})).is_ok());
    }
}
