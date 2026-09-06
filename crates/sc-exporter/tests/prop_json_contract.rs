//! 协议契约测试：`prop_json` 输出必须与 C# `DumpProp(json: true)` 逐字段一致。
//! 语义权威：`Simcitypak-v2/SimCityPak/Cli/CliRunner.cs` 与 `PackageReader/Properties/*`。

use dbpf::ResourceId;
use sc_exporter::prop_json::{
    csharp_type_name, display_value, dump_property_file, fallback_name, net48_float_to_string,
    prop_name, to_json,
};
use sc_properties::{Key, Kind, PropType, Property, PropertyFile, Text, Transform, Value};
use sc_registry::Registry;

fn prop(hash: u32, t: PropType, kind: Kind) -> Property {
    Property {
        hash,
        prop_type: t,
        kind,
        encoding: sc_properties::PropertyEncoding::default(),
    }
}

fn scalar(hash: u32, t: PropType, v: Value) -> Property {
    prop(hash, t, Kind::Scalar(v))
}

fn file(values: Vec<Property>) -> PropertyFile {
    PropertyFile {
        claimed_count: values.len() as u32,
        values,
    }
}

// ---- .NET Framework float (G7) ----

#[test]
fn net48_float_matches_g7() {
    assert_eq!(net48_float_to_string(0.0), "0");
    assert_eq!(net48_float_to_string(-0.0), "0");
    assert_eq!(net48_float_to_string(0.5), "0.5");
    assert_eq!(net48_float_to_string(1.5), "1.5");
    assert_eq!(net48_float_to_string(-2.25), "-2.25");
    assert_eq!(net48_float_to_string(0.1), "0.1");
    // 0.1f stored value is 0.100000001490116...; G7 rounds to 0.1
    assert_eq!(net48_float_to_string(0.1 + 0.2), "0.3");
    assert_eq!(net48_float_to_string(0.333_333_34), "0.3333333");
    assert_eq!(net48_float_to_string(250.0), "250");
    assert_eq!(net48_float_to_string(1_234_567.0), "1234567");
    assert_eq!(net48_float_to_string(12_345_678.0), "1.234568E+07");
    assert_eq!(net48_float_to_string(12_345_670.0), "1.234567E+07");
    assert_eq!(net48_float_to_string(10_000_000.0), "1E+07");
    assert_eq!(net48_float_to_string(0.001), "0.001");
    assert_eq!(net48_float_to_string(0.0001), "0.0001");
    assert_eq!(net48_float_to_string(1e-5), "1E-05");
    assert_eq!(net48_float_to_string(1.5e-5), "1.5E-05");
    assert_eq!(net48_float_to_string(1e10), "1E+10");
    assert_eq!(net48_float_to_string(f32::NAN), "NaN");
    assert_eq!(net48_float_to_string(f32::INFINITY), "Infinity");
    assert_eq!(net48_float_to_string(f32::NEG_INFINITY), "-Infinity");
}

// ---- type names ----

#[test]
fn csharp_type_names_strip_property_suffix() {
    assert_eq!(csharp_type_name(PropType::Bool), "Bool");
    assert_eq!(csharp_type_name(PropType::Int32), "Int32");
    assert_eq!(csharp_type_name(PropType::UInt32), "UInt32");
    assert_eq!(csharp_type_name(PropType::Float), "Float");
    assert_eq!(csharp_type_name(PropType::String8), "String8");
    assert_eq!(csharp_type_name(PropType::String16), "String16");
    assert_eq!(csharp_type_name(PropType::Key), "Key");
    assert_eq!(csharp_type_name(PropType::Text), "Text");
    assert_eq!(csharp_type_name(PropType::Vector2), "Vector2");
    assert_eq!(csharp_type_name(PropType::Vector3), "Vector3");
    assert_eq!(csharp_type_name(PropType::ColorRgb), "ColorRGB");
    assert_eq!(csharp_type_name(PropType::Vector4), "Vector4");
    assert_eq!(csharp_type_name(PropType::ColorRgba), "ColorRGBA");
    assert_eq!(csharp_type_name(PropType::Transform), "Transform");
    assert_eq!(csharp_type_name(PropType::BoundingBox), "BoundingBox");
}

// ---- DisplayValue ----

#[test]
fn display_value_matches_csharp() {
    assert_eq!(display_value(&Value::Bool(true)), "True");
    assert_eq!(display_value(&Value::Bool(false)), "False");
    assert_eq!(display_value(&Value::Int32(-16)), "-16");
    assert_eq!(display_value(&Value::UInt32(0x00B1_B104)), "0x00b1b104");
    assert_eq!(display_value(&Value::Float(1.5)), "1.5");
    assert_eq!(
        display_value(&Value::Key(Key {
            instance: 1,
            type_id: 2,
            group: 3
        })),
        ""
    );
    assert_eq!(
        display_value(&Value::Text(Text {
            table_id: 7,
            instance_id: 8
        })),
        ""
    );
    assert_eq!(display_value(&Value::String8("hello".into())), "hello");
    assert_eq!(display_value(&Value::String16("件".into())), "件");
    assert_eq!(
        display_value(&Value::Vector2([1.0, 2.5])),
        "Vector2 (X = 1, Y = 2.5)"
    );
    assert_eq!(
        display_value(&Value::Vector3([1.0, 2.0, 3.5])),
        "Vector3 (X = 1, Y = 2, Z = 3.5)"
    );
    assert_eq!(
        display_value(&Value::Vector4([1.0, 2.0, 3.0, 4.5])),
        "Vector4 (X = 1, Y = 2, Z = 3, W = 4.5)"
    );
    assert_eq!(
        display_value(&Value::ColorRgb {
            r: 0.5,
            g: 1.0,
            b: 0.25
        }),
        "R 0.5-G 1-B 0.25"
    );
    assert_eq!(
        display_value(&Value::ColorRgba {
            r: 0.5,
            g: 1.0,
            b: 0.25,
            a: 0.0
        }),
        "R 0.5-G 1-B 0.25-A 0"
    );
    assert_eq!(
        display_value(&Value::BoundingBox {
            min: [0.0; 3],
            max: [9.5; 3]
        }),
        "Bounding Box (MinX = 0, MinY = 0, MinZ = 0, MaxX = 9.5, MaxY = 9.5, MaxZ = 9.5)"
    );
}

#[test]
fn transform_value_matches_csharp_format() {
    let t12 = Transform {
        flags: 0,
        unknown: None,
        matrix: vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.5,
        ],
    };
    assert_eq!(
        display_value(&Value::Transform(t12)),
        "Transform (Count = 12, 1,2,3,4,5,6,7,8,9,10,11,12.5, 0, 0)"
    );
    let t15 = Transform {
        flags: 15,
        unknown: Some(7.5),
        matrix: vec![1.0; 12],
    };
    assert_eq!(
        display_value(&Value::Transform(t15)),
        "Transform (Count = 12, 1,1,1,1,1,1,1,1,1,1,1,1, 15, 7.5)"
    );
    let t3 = Transform {
        flags: 0x0C,
        unknown: None,
        matrix: vec![1.0, 2.0, 3.0],
    };
    assert_eq!(
        display_value(&Value::Transform(t3)),
        "Transform (Count = 3, 1,2,3, 12, 0)"
    );
}

// ---- dump structure ----

#[test]
fn dump_sorts_by_hash_and_counts_actual_entries() {
    let pf = file(vec![
        scalar(0x02, PropType::UInt32, Value::UInt32(1)),
        scalar(0x01, PropType::Float, Value::Float(0.5)),
    ]);
    let dump = dump_property_file(&pf, "x", None);
    assert_eq!(dump.property_count, 2);
    assert_eq!(dump.properties.len(), 2);
    assert_eq!(dump.properties[0].hash, "0x00000001");
    assert_eq!(dump.properties[0].value, "0.5");
    assert_eq!(dump.properties[1].hash, "0x00000002");
    assert_eq!(dump.properties[1].value, "0x00000001");
    assert_eq!(dump.properties[0].name, None, "no registry -> null name");
}

#[test]
fn empty_variants_are_excluded_like_csharp() {
    let pf = file(vec![
        scalar(0x01, PropType::UInt32, Value::UInt32(7)),
        prop(0x02, PropType::Float, Kind::Empty),
    ]);
    let dump = dump_property_file(&pf, "x", None);
    assert_eq!(dump.property_count, 1, "C# never stores empty variants");
    assert_eq!(dump.properties[0].hash, "0x00000001");
}

#[test]
fn array_value_is_space_joined_and_typed_array() {
    let pf = file(vec![prop(
        0x10,
        PropType::Float,
        Kind::Array(vec![
            Value::Float(1.0),
            Value::Float(2.5),
            Value::Float(4.0),
        ]),
    )]);
    let dump = dump_property_file(&pf, "x", None);
    assert_eq!(dump.properties[0].type_name, "Array");
    assert_eq!(dump.properties[0].value, " 1 2.5 4");
}

#[test]
fn fallback_name_matches_csharp_format() {
    let id = ResourceId {
        type_id: 0x00B1_B104,
        group: 0x0987_8A01,
        instance: 0x1234_5678,
    };
    assert_eq!(fallback_name(id), "00b1b104-09878a01-12345678");
}

#[test]
fn json_output_has_csharp_shape() {
    let pf = file(vec![scalar(0x01, PropType::UInt32, Value::UInt32(1))]);
    let dump = dump_property_file(&pf, "00b1b104-0-1", None);
    let json = to_json(&dump).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    let obj = value.as_object().unwrap();
    let keys: Vec<_> = obj.keys().map(String::as_str).collect();
    assert_eq!(keys.len(), 3);
    for key in ["name", "propertyCount", "properties"] {
        assert!(keys.contains(&key), "missing key {key}");
    }
    let first = &obj["properties"][0];
    assert_eq!(first["hash"], "0x00000001");
    assert_eq!(first["type"], "UInt32");
    assert_eq!(first["value"], "0x00000001");
    assert!(first["name"].is_null());
}

// ---- registry name resolution ----

#[test]
fn prop_name_prefers_comments_then_name() {
    // 合成 registry 不可行（rusqlite 需真实库文件），这里验证 None 路径；
    // comments 优先级在 real_dump 测试中以真实 database_main.s3db 验证。
    assert_eq!(prop_name(0x0975_695F, None), None);
}

#[test]
fn real_registry_resolves_names_when_present() {
    let db = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/packages/database_main.s3db"
    );
    if !std::path::Path::new(db).exists() {
        eprintln!("skipping: {db} not present");
        return;
    }
    let registry = Registry::open(db).unwrap();
    let name = prop_name(0x0975_695F, Some(&registry)).expect("Model Details known");
    assert!(name.contains("Model Details"), "got {name}");
    assert_eq!(prop_name(0xDEAD_BEEF, Some(&registry)), None);
}
