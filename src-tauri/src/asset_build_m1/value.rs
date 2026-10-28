use crate::asset_build_m0::parse_tgi;
use sc_properties::{Key, PropType, Transform, Value};
use serde_json::Value as JsonValue;

pub(super) fn json_to_value(
    prop_type: PropType,
    value: &JsonValue,
    hash: u32,
) -> Result<Value, String> {
    let error = || format!("value for 0x{hash:08X} does not match {}", prop_type.name());
    match prop_type {
        PropType::Bool => value.as_bool().map(Value::Bool).ok_or_else(error),
        PropType::Int32 => value
            .as_i64()
            .and_then(|value| i32::try_from(value).ok())
            .map(Value::Int32)
            .ok_or_else(error),
        PropType::UInt32 => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .map(Value::UInt32)
            .ok_or_else(error),
        PropType::Float => value
            .as_f64()
            .filter(|value| value.is_finite() && (*value as f32).is_finite())
            .map(|value| Value::Float(value as f32))
            .ok_or_else(error),
        PropType::String8 => value
            .as_str()
            .map(|value| Value::String8(value.into()))
            .ok_or_else(error),
        PropType::String16 => value
            .as_str()
            .map(|value| Value::String16(value.into()))
            .ok_or_else(error),
        PropType::Key => value
            .as_str()
            .ok_or_else(error)
            .and_then(parse_tgi)
            .map(|key| {
                Value::Key(Key {
                    instance: key.instance,
                    type_id: key.type_id,
                    group: key.group,
                })
            }),
        PropType::Text => {
            let object = value.as_object().ok_or_else(error)?;
            let table_id =
                json_u32(object.get("tableId").ok_or_else(error)?).map_err(|_| error())?;
            let instance_id =
                json_u32(object.get("instanceId").ok_or_else(error)?).map_err(|_| error())?;
            Ok(Value::Text(sc_properties::Text {
                table_id,
                instance_id,
            }))
        }
        PropType::Vector2 => array::<2>(value, error).map(Value::Vector2),
        PropType::Vector3 => array::<3>(value, error).map(Value::Vector3),
        PropType::Vector4 => array::<4>(value, error).map(Value::Vector4),
        PropType::ColorRgb => array::<3>(value, error).map(|[r, g, b]| Value::ColorRgb { r, g, b }),
        PropType::ColorRgba => {
            array::<4>(value, error).map(|[r, g, b, a]| Value::ColorRgba { r, g, b, a })
        }
        PropType::BoundingBox => value
            .as_object()
            .and_then(|object| {
                Some((
                    array::<3>(object.get("min")?, error).ok()?,
                    array::<3>(object.get("max")?, error).ok()?,
                ))
            })
            .map(|(min, max)| Value::BoundingBox { min, max })
            .ok_or_else(error),
        PropType::Transform => value
            .as_object()
            .and_then(|object| {
                let flags = json_u16(object.get("flags")?).ok()?;
                let matrix = object
                    .get("matrix")?
                    .as_array()?
                    .iter()
                    .map(|v| {
                        v.as_f64()
                            .filter(|v| v.is_finite() && (*v as f32).is_finite())
                            .map(|v| v as f32)
                    })
                    .collect::<Option<Vec<_>>>()?;
                let unknown = match object.get("unknown") {
                    None | Some(JsonValue::Null) => None,
                    Some(value) => Some(
                        value
                            .as_f64()
                            .filter(|v| v.is_finite() && (*v as f32).is_finite())?
                            as f32,
                    ),
                };
                let count = match flags {
                    12 => 3,
                    13 => 4,
                    _ => 12,
                };
                if matrix.len() != count || (flags == 15) != unknown.is_some() {
                    return None;
                }
                Some(Value::Transform(Transform {
                    flags,
                    unknown,
                    matrix,
                }))
            })
            .ok_or_else(error),
    }
}

fn array<const N: usize>(
    value: &JsonValue,
    error: impl Fn() -> String,
) -> Result<[f32; N], String> {
    let values = value.as_array().ok_or_else(&error)?;
    if values.len() != N {
        return Err(error());
    }
    let values = values
        .iter()
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite() && (*value as f32).is_finite())
                .map(|value| value as f32)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(&error)?;
    values.try_into().map_err(|_| error())
}

fn json_u32(value: &JsonValue) -> Result<u32, String> {
    value
        .as_u64()
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| "expected uint32".into())
}

fn json_u16(value: &JsonValue) -> Result<u16, String> {
    value
        .as_u64()
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| "expected uint16".into())
}
