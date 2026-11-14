use super::value::json_to_value;
use dbpf::ResourceId;
use sc_properties::{Kind, PropType, Property, PropertyEncoding, PropertyFile, Value};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PatchOperation {
    Set,
    Remove,
}

impl Default for PatchOperation {
    fn default() -> Self {
        Self::Set
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PropertyPatchSpec {
    pub tgi: String,
    pub hash: String,
    #[serde(default)]
    pub operation: PatchOperation,
    pub index: Option<usize>,
    #[serde(
        default,
        deserialize_with = "present_json",
        skip_serializing_if = "Option::is_none"
    )]
    pub value: Option<JsonValue>,
    #[serde(
        default,
        deserialize_with = "present_json",
        skip_serializing_if = "Option::is_none"
    )]
    pub expected: Option<JsonValue>,
    pub property_type: Option<String>,
    #[serde(default)]
    pub array: bool,
}

#[derive(Debug, Clone)]
pub struct PropertyPatch {
    pub target: ResourceId,
    pub hash: u32,
    pub operation: PatchOperation,
    pub index: Option<usize>,
    pub value: Option<JsonValue>,
    pub expected: Option<JsonValue>,
    pub property_type: Option<PropType>,
    pub array: bool,
}

pub fn parse_patch(spec: PropertyPatchSpec) -> Result<PropertyPatch, String> {
    let target = parse_tgi(&spec.tgi)?;
    if target.type_id != sc_properties::PROPERTY_RESOURCE_TYPE {
        return Err("Property processor requires type 00B1B104".into());
    }
    if spec.array && spec.index.is_some() {
        return Err("array=true cannot be combined with index".into());
    }
    match spec.operation {
        PatchOperation::Set if spec.value.is_none() => return Err("set requires value".into()),
        PatchOperation::Remove if spec.value.is_some() || spec.array => {
            return Err("remove does not accept value or array=true".into());
        }
        _ => {}
    }
    Ok(PropertyPatch {
        target,
        hash: parse_hex_u32(&spec.hash, "property hash")?,
        operation: spec.operation,
        index: spec.index,
        value: spec.value,
        expected: spec.expected,
        property_type: spec
            .property_type
            .as_deref()
            .map(parse_prop_type)
            .transpose()?,
        array: spec.array,
    })
}

/// Pure transformation: validation failures cannot partially mutate the input.
pub fn apply_patch(file: &PropertyFile, patch: &PropertyPatch) -> Result<PropertyFile, String> {
    let mut result = file.clone();
    let position = result.values.iter().position(|p| p.hash == patch.hash);
    let existing = position.map(|i| &result.values[i]);
    let prop_type = match (existing, patch.property_type) {
        (Some(property), Some(requested)) if property.prop_type != requested => {
            return Err(format!("property 0x{:08X} type mismatch", patch.hash));
        }
        (Some(property), _) => property.prop_type,
        (None, Some(requested)) => requested,
        (None, None) => {
            return Err(format!(
                "new property 0x{:08X} requires propertyType",
                patch.hash
            ));
        }
    };
    if patch.array && patch.index.is_some() {
        return Err("array=true cannot be combined with index".into());
    }
    if let Some(expected) = &patch.expected {
        let property = existing.ok_or("expected precondition requires an existing property")?;
        let actual = selected_kind(property, patch.index)?;
        let expected = value_to_kind(
            prop_type,
            expected,
            matches!(actual, Kind::Array(_)),
            patch.hash,
        )?;
        if actual != expected {
            return Err(format!(
                "property 0x{:08X} expected value mismatch",
                patch.hash
            ));
        }
    }
    match patch.operation {
        PatchOperation::Remove => {
            let position = position.ok_or("cannot remove a missing property")?;
            if let Some(index) = patch.index {
                match &mut result.values[position].kind {
                    Kind::Array(values) if index < values.len() => {
                        values.remove(index);
                    }
                    _ => return Err("remove index must address an existing array element".into()),
                }
            } else {
                result.values.remove(position);
            }
        }
        PatchOperation::Set => {
            let raw = patch.value.as_ref().ok_or("set operation requires value")?;
            if let Some(index) = patch.index {
                let position = position.ok_or("cannot index a missing property")?;
                match &mut result.values[position].kind {
                    Kind::Array(values) if index < values.len() => {
                        values[index] = json_to_value(prop_type, raw, patch.hash)?;
                    }
                    _ => return Err("set index must address an existing array element".into()),
                }
            } else {
                let kind = value_to_kind(prop_type, raw, patch.array, patch.hash)?;
                if let Some(position) = position {
                    let property = &mut result.values[position];
                    if matches!(property.kind, Kind::Array(_)) && !patch.array && !raw.is_null() {
                        return Err("array replacement requires array=true".into());
                    }
                    // Retain array strides and flags when the shape stays the same.
                    if std::mem::discriminant(&property.kind) != std::mem::discriminant(&kind) {
                        property.encoding = encoding_for(prop_type, &kind);
                    }
                    property.kind = kind;
                } else {
                    result.values.push(Property {
                        hash: patch.hash,
                        prop_type,
                        encoding: encoding_for(prop_type, &kind),
                        kind,
                    });
                }
            }
        }
    }
    result.claimed_count = u32::try_from(result.values.len()).map_err(|e| e.to_string())?;
    Ok(result)
}

fn selected_kind(property: &Property, index: Option<usize>) -> Result<Kind, String> {
    match (&property.kind, index) {
        (kind, None) => Ok(kind.clone()),
        (Kind::Array(values), Some(index)) => values
            .get(index)
            .cloned()
            .map(Kind::Scalar)
            .ok_or("expected index out of range".into()),
        _ => Err("expected index requires an array".into()),
    }
}

fn encoding_for(prop_type: PropType, kind: &Kind) -> PropertyEncoding {
    let stride = match prop_type {
        PropType::Bool => 1,
        PropType::Int32 | PropType::UInt32 | PropType::Float => 4,
        PropType::Key | PropType::Vector3 | PropType::ColorRgb => 12,
        PropType::Text => sc_properties::TEXT_ARRAY_ITEM_SIZE,
        PropType::Vector2 => 8,
        PropType::Vector4 | PropType::ColorRgba => 16,
        PropType::BoundingBox => 24,
        PropType::String8 | PropType::String16 | PropType::Transform => 0,
    };
    PropertyEncoding {
        flags: 0,
        array_item_size: matches!(kind, Kind::Array(_)).then_some(stride),
    }
}

fn present_json<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<JsonValue>, D::Error> {
    JsonValue::deserialize(deserializer).map(Some)
}

fn value_to_kind(
    prop_type: PropType,
    value: &JsonValue,
    array: bool,
    hash: u32,
) -> Result<Kind, String> {
    if value == &JsonValue::Null {
        return Ok(Kind::Empty);
    }
    if array {
        return Ok(Kind::Array(json_to_array(prop_type, value, hash)?));
    }
    Ok(Kind::Scalar(json_to_value(prop_type, value, hash)?))
}

fn json_to_array(prop_type: PropType, value: &JsonValue, hash: u32) -> Result<Vec<Value>, String> {
    value
        .as_array()
        .ok_or_else(|| format!("array value for 0x{hash:08X} must be a JSON array"))?
        .iter()
        .map(|item| json_to_value(prop_type, item, hash))
        .collect()
}

fn parse_prop_type(value: &str) -> Result<PropType, String> {
    match value.to_ascii_lowercase().as_str() {
        "bool" => Ok(PropType::Bool),
        "int32" => Ok(PropType::Int32),
        "uint32" => Ok(PropType::UInt32),
        "float" => Ok(PropType::Float),
        "string8" => Ok(PropType::String8),
        "string16" => Ok(PropType::String16),
        "key" => Ok(PropType::Key),
        "text" => Ok(PropType::Text),
        "vector2" => Ok(PropType::Vector2),
        "vector3" => Ok(PropType::Vector3),
        "colorrgb" => Ok(PropType::ColorRgb),
        "vector4" => Ok(PropType::Vector4),
        "colorrgba" => Ok(PropType::ColorRgba),
        "transform" => Ok(PropType::Transform),
        "bbox" | "boundingbox" => Ok(PropType::BoundingBox),
        _ => Err(format!("unsupported property type {value}")),
    }
}

pub(super) fn parse_tgi(value: &str) -> Result<ResourceId, String> {
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != 3 {
        return Err(format!("invalid TGI {value}"));
    }
    let mut numbers = [0u32; 3];
    for (index, part) in parts.iter().enumerate() {
        numbers[index] = parse_hex_u32(part, "TGI component")?;
    }
    Ok(ResourceId {
        type_id: numbers[0],
        group: numbers[1],
        instance: numbers[2],
    })
}

fn parse_hex_u32(value: &str, label: &str) -> Result<u32, String> {
    u32::from_str_radix(value.trim().trim_start_matches("0x"), 16)
        .map_err(|_| format!("invalid {label} {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_properties::Value;

    fn property(value: i32) -> PropertyFile {
        PropertyFile {
            claimed_count: 1,
            values: vec![Property {
                hash: 0x1234,
                prop_type: PropType::Int32,
                kind: Kind::Scalar(Value::Int32(value)),
                encoding: PropertyEncoding::default(),
            }],
        }
    }

    #[test]
    fn patches_scalar_with_expected_old_value() {
        let file = property(10);
        let patch = parse_patch(PropertyPatchSpec {
            tgi: "00B1B104:40E0C000:8D007745".into(),
            hash: "0x1234".into(),
            operation: PatchOperation::Set,
            index: None,
            value: Some(serde_json::json!(20)),
            expected: Some(serde_json::json!(10)),
            property_type: None,
            array: false,
        })
        .unwrap();
        let updated = apply_patch(&file, &patch).unwrap();
        assert_eq!(updated.values[0].scalar(), Some(&Value::Int32(20)));
        assert_eq!(file.values[0].scalar(), Some(&Value::Int32(10)));
    }

    #[test]
    fn rejects_stale_expected_value() {
        let file = property(11);
        let patch = parse_patch(PropertyPatchSpec {
            tgi: "00B1B104:40E0C000:8D007745".into(),
            hash: "1234".into(),
            operation: PatchOperation::Set,
            index: None,
            value: Some(serde_json::json!(20)),
            expected: Some(serde_json::json!(10)),
            property_type: None,
            array: false,
        })
        .unwrap();
        assert!(apply_patch(&file, &patch).is_err());
    }

    fn spec(mut fields: JsonValue) -> PropertyPatch {
        fields["tgi"] = serde_json::json!("00B1B104:40E0C000:8D007745");
        fields["hash"] = serde_json::json!("1234");
        parse_patch(serde_json::from_value(fields).unwrap()).unwrap()
    }

    #[test]
    fn arrays_nulls_and_guarded_removals_roundtrip() {
        let file = PropertyFile::default();
        let create =
            spec(serde_json::json!({"propertyType": "int32", "array": true, "value": [10, 20]}));
        let array = apply_patch(&file, &create).unwrap();
        let reparsed = PropertyFile::parse(&array.encode_canonical().unwrap()).unwrap();
        assert_eq!(reparsed.values[0].encoding.array_item_size, Some(4));
        let stale_remove =
            spec(serde_json::json!({"operation": "remove", "index": 1, "expected": 30}));
        assert!(apply_patch(&array, &stale_remove).is_err());
        let remove = spec(serde_json::json!({"operation": "remove", "index": 1, "expected": 20}));
        let reduced = apply_patch(&array, &remove).unwrap();
        assert_eq!(reduced.values[0].kind, Kind::Array(vec![Value::Int32(10)]));
        let replace =
            spec(serde_json::json!({"array": true, "expected": [10], "value": [30, 40, 50]}));
        let updated = apply_patch(&reduced, &replace).unwrap();
        assert_eq!(updated.values[0].array().unwrap().len(), 3);
        let empty = apply_patch(&updated, &spec(serde_json::json!({"value": null}))).unwrap();
        assert_eq!(empty.values[0].kind, Kind::Empty);
        let scalar = apply_patch(
            &empty,
            &spec(serde_json::json!({"expected": null, "value": 70})),
        )
        .unwrap();
        let scalar = PropertyFile::parse(&scalar.encode_canonical().unwrap()).unwrap();
        assert_eq!(scalar.values[0].scalar(), Some(&Value::Int32(70)));
        assert_eq!(array.values[0].array().unwrap().len(), 2);
        let removed = apply_patch(
            &scalar,
            &spec(serde_json::json!({"operation": "remove", "expected": 70})),
        )
        .unwrap();
        assert!(
            PropertyFile::parse(&removed.encode_canonical().unwrap())
                .unwrap()
                .values
                .is_empty()
        );
    }

    #[test]
    fn supported_value_types_roundtrip_with_typed_arrays() {
        let samples = serde_json::json!([
            ["bool", true], ["int32", -2147483648_i64], ["uint32", 4294967295_u64],
            ["float", 1.25], ["string8", "building"], ["string16", "威利斯"],
            ["key", "2F4E681B:00000000:10569AF0"], ["text", {"tableId": 1, "instanceId": 2}],
            ["vector2", [1, 2]], ["vector3", [1, 2, 3]], ["vector4", [1, 2, 3, 4]],
            ["colorrgb", [0.1, 0.2, 0.3]], ["colorrgba", [0.1, 0.2, 0.3, 1]],
            ["bbox", {"min": [0, 0, 0], "max": [80, 80, 219]}],
            ["transform", {"flags": 12, "matrix": [1, 2, 3]}]
        ]);
        for sample in samples.as_array().unwrap() {
            for is_array in [false, true] {
                let value = if is_array {
                    serde_json::json!([sample[1]])
                } else {
                    sample[1].clone()
                };
                let patch = spec(
                    serde_json::json!({"propertyType": sample[0], "array": is_array, "value": value}),
                );
                let result = apply_patch(&PropertyFile::default(), &patch).unwrap();
                let encoded = result.encode_canonical().unwrap();
                let readback = PropertyFile::parse(&encoded).unwrap();
                assert_eq!(readback.values[0].kind, result.values[0].kind, "{sample}");
                if is_array && sample[0] == "text" {
                    assert_eq!(readback.values[0].encoding.array_item_size, Some(520));
                }
            }
        }
    }

    #[test]
    fn rejects_type_mismatch_numeric_overflow_and_invalid_transforms() {
        let file = property(10);
        assert!(
            apply_patch(
                &file,
                &spec(serde_json::json!({"propertyType": "float", "value": 1}))
            )
            .is_err()
        );
        for (ty, value) in [
            ("int32", serde_json::json!(2147483648_i64)),
            ("uint32", serde_json::json!(-1)),
            ("float", serde_json::json!(1e100)),
            ("vector3", serde_json::json!([1e100, 0, 0])),
            (
                "transform",
                serde_json::json!({"flags": 12, "matrix": [1, 2]}),
            ),
            (
                "transform",
                serde_json::json!({"flags": 12, "matrix": [1e100, 2, 3]}),
            ),
        ] {
            let patch = spec(serde_json::json!({"propertyType": ty, "value": value}));
            assert!(
                apply_patch(&PropertyFile::default(), &patch).is_err(),
                "{ty}"
            );
        }
        assert_eq!(file, property(10));
    }
}
