use super::*;

fn be16(v: u16) -> [u8; 2] {
    v.to_be_bytes()
}

fn be32(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

fn push_f32(v: &mut Vec<u8>, x: f32) {
    v.extend_from_slice(&x.to_be_bytes());
}

fn push_u32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_be_bytes());
}

/// Scalar entry: hash + type + flags + payload.
fn entry(hash: u32, file_type: u16, flags: u16, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&be32(hash));
    v.extend_from_slice(&be16(file_type));
    v.extend_from_slice(&be16(flags));
    v.extend_from_slice(payload);
    v
}

fn file(entries: &[Vec<u8>]) -> Vec<u8> {
    let mut v = be32(entries.len() as u32).to_vec();
    for e in entries {
        v.extend_from_slice(e);
    }
    v
}

#[test]
fn parse_limits_reject_large_entry_counts() {
    let data = be32(2).to_vec();
    assert!(matches!(
        PropertyFile::parse_with_limits(
            &data,
            ParseLimits {
                max_entries: 1,
                ..ParseLimits::default()
            }
        ),
        Err(Error::LimitExceeded("entry count"))
    ));
}

#[test]
fn scalar_types_roundtrip() {
    let mut key = Vec::new();
    push_u32(&mut key, 0xAA);
    push_u32(&mut key, 0xBB);
    push_u32(&mut key, 0xCC);
    let mut text = Vec::new();
    push_u32(&mut text, 7);
    push_u32(&mut text, 8);
    let mut s8 = Vec::new();
    push_u32(&mut s8, 5);
    s8.extend_from_slice(b"hello");
    let mut s16 = Vec::new();
    push_u32(&mut s16, 1);
    s16.extend_from_slice(&[0x4E, 0xF6]); // "件" UTF-16BE

    let data = file(&[
        entry(0x0000_0001, 1, 0, &[1]),               // bool true
        entry(0x0000_0002, 9, 0, &be32(0xFFFF_FFF0)), // int32
        entry(0x0000_0003, 10, 0, &be32(42)),         // uint32
        {
            let mut p = Vec::new();
            push_f32(&mut p, 1.5);
            entry(0x0000_0004, 13, 0, &p) // float
        },
        entry(0x0000_0005, 32, 0, &key),
        entry(0x0000_0006, 34, 0, &text),
        entry(0x0000_0007, 18, 0, &s8),
        entry(0x0000_0008, 19, 0, &s16),
    ]);
    let pf = PropertyFile::parse(&data).unwrap();
    assert_eq!(pf.claimed_count, 8);
    assert_eq!(pf.values.len(), 8);

    assert_eq!(pf.get(1).unwrap().scalar(), Some(&Value::Bool(true)));
    assert_eq!(pf.get(2).unwrap().scalar(), Some(&Value::Int32(-16)));
    assert_eq!(pf.get(3).unwrap().scalar(), Some(&Value::UInt32(42)));
    assert_eq!(pf.get(4).unwrap().scalar(), Some(&Value::Float(1.5)));
    match pf.get(5).unwrap().scalar() {
        Some(Value::Key(value)) => {
            assert_eq!(value.instance, 0xAA);
            assert_eq!(value.type_id, 0xBB);
            assert_eq!(value.group, 0xCC);
        }
        other => panic!("expected key, got {other:?}"),
    }
    match pf.get(6).unwrap().scalar() {
        Some(Value::Text(value)) => {
            assert_eq!(value.table_id, 7);
            assert_eq!(value.instance_id, 8);
        }
        other => panic!("expected text, got {other:?}"),
    }
    assert_eq!(
        pf.get(7).unwrap().scalar(),
        Some(&Value::String8("hello".into()))
    );
    assert_eq!(
        pf.get(8).unwrap().scalar(),
        Some(&Value::String16("件".into()))
    );
}

#[test]
fn float_vector_types() {
    let mut v2 = Vec::new();
    push_f32(&mut v2, 1.0);
    push_f32(&mut v2, 2.0);
    let mut v3 = Vec::new();
    push_f32(&mut v3, 1.0);
    push_f32(&mut v3, 2.0);
    push_f32(&mut v3, 3.0);
    let mut v4 = Vec::new();
    push_f32(&mut v4, 1.0);
    push_f32(&mut v4, 2.0);
    push_f32(&mut v4, 3.0);
    push_f32(&mut v4, 4.0);
    let mut bbox = Vec::new();
    for x in [0.0f32, 0.0, 0.0, 9.0, 9.0, 9.0] {
        push_f32(&mut bbox, x);
    }

    let data = file(&[
        entry(0x10, 48, 0, &v2),
        entry(0x11, 49, 0, &v3),
        entry(0x12, 50, 0, &v3),
        entry(0x13, 51, 0, &v4),
        entry(0x14, 52, 0, &v4),
        entry(0x15, 57, 0, &bbox),
    ]);
    let pf = PropertyFile::parse(&data).unwrap();
    assert_eq!(
        pf.get(0x10).unwrap().scalar(),
        Some(&Value::Vector2([1.0, 2.0]))
    );
    assert_eq!(
        pf.get(0x11).unwrap().scalar(),
        Some(&Value::Vector3([1.0, 2.0, 3.0]))
    );
    assert_eq!(
        pf.get(0x12).unwrap().scalar(),
        Some(&Value::ColorRgb {
            r: 1.0,
            g: 2.0,
            b: 3.0
        })
    );
    assert_eq!(
        pf.get(0x13).unwrap().scalar(),
        Some(&Value::Vector4([1.0, 2.0, 3.0, 4.0]))
    );
    assert_eq!(
        pf.get(0x14).unwrap().scalar(),
        Some(&Value::ColorRgba {
            r: 1.0,
            g: 2.0,
            b: 3.0,
            a: 4.0
        })
    );
    assert_eq!(
        pf.get(0x15).unwrap().scalar(),
        Some(&Value::BoundingBox {
            min: [0.0; 3],
            max: [9.0; 3]
        })
    );
}

#[test]
fn transform_variants() {
    // default: 12 floats
    let mut payload12 = be16(0).to_vec();
    for i in 0..12 {
        push_f32(&mut payload12, i as f32);
    }
    // flags 0x0C: 3 floats
    let mut payload3 = be16(0x0C).to_vec();
    push_f32(&mut payload3, 1.0);
    push_f32(&mut payload3, 2.0);
    push_f32(&mut payload3, 3.0);
    // flags 15: unknown float then 12 floats
    let mut payload15 = be16(15).to_vec();
    push_f32(&mut payload15, 7.0);
    for i in 0..12 {
        push_f32(&mut payload15, i as f32);
    }

    let data = file(&[
        entry(0x20, 56, 0, &payload12),
        entry(0x21, 56, 0, &payload3),
        entry(0x22, 56, 0, &payload15),
    ]);
    let pf = PropertyFile::parse(&data).unwrap();

    match pf.get(0x20).unwrap().scalar() {
        Some(Value::Transform(t)) => {
            assert_eq!(t.flags, 0);
            assert_eq!(t.unknown, None);
            assert_eq!(t.matrix.len(), 12);
        }
        other => panic!("expected transform, got {other:?}"),
    }
    match pf.get(0x21).unwrap().scalar() {
        Some(Value::Transform(t)) => {
            assert_eq!(t.flags, 0x0C);
            assert_eq!(t.matrix, vec![1.0, 2.0, 3.0]);
        }
        other => panic!("expected transform, got {other:?}"),
    }
    match pf.get(0x22).unwrap().scalar() {
        Some(Value::Transform(t)) => {
            assert_eq!(t.flags, 15);
            assert_eq!(t.unknown, Some(7.0));
            assert_eq!(t.matrix.len(), 12);
        }
        other => panic!("expected transform, got {other:?}"),
    }
}

#[test]
fn array_entries() {
    // variant flags with array bit: 0x30 set, 0x40 clear (e.g. 0x8090).
    let mut payload = Vec::new();
    push_u32(&mut payload, 3); // count
    push_u32(&mut payload, 4); // item size
    for x in [1.0f32, 2.0, 3.0] {
        push_f32(&mut payload, x);
    }

    let data = file(&[entry(0x30, 13, 0x8090, &payload)]);
    let pf = PropertyFile::parse(&data).unwrap();

    let p = pf.get(0x30).unwrap();
    assert_eq!(p.prop_type, PropType::Float);
    assert_eq!(
        p.array(),
        Some([Value::Float(1.0), Value::Float(2.0), Value::Float(3.0)].as_slice())
    );
}

#[test]
fn empty_variant_entry() {
    // flags & 0x30 != 0 && flags & 0x40 != 0 -> no payload at all.
    let data = file(&[entry(0x40, 13, 0x8070, &[])]);
    let pf = PropertyFile::parse(&data).unwrap();
    let p = pf.get(0x40).unwrap();
    assert_eq!(p.kind, Kind::Empty);
}

#[test]
fn string8_zero_length_with_flag_reads_u8_length() {
    // length u32 = 0, flag 0x100 set -> one-byte length follows.
    let mut payload = be32(0).to_vec();
    payload.push(5);
    payload.extend_from_slice(b"short");
    let data = file(&[entry(0x50, 18, 0x0100, &payload)]);
    let pf = PropertyFile::parse(&data).unwrap();
    assert_eq!(
        pf.get(0x50).unwrap().scalar(),
        Some(&Value::String8("short".into()))
    );
}

#[test]
fn claimed_count_exceeding_entries_is_tolerated() {
    // header claims 3, only 1 entry present (known real-world quirk).
    let mut data = be32(3).to_vec();
    data.extend_from_slice(&entry(0x60, 10, 0, &be32(7)));
    let pf = PropertyFile::parse(&data).unwrap();
    assert_eq!(pf.claimed_count, 3);
    assert_eq!(pf.values.len(), 1);
}

#[test]
fn duplicate_hash_rejected() {
    let data = file(&[entry(0x70, 10, 0, &be32(1)), entry(0x70, 10, 0, &be32(2))]);
    assert!(matches!(
        PropertyFile::parse(&data),
        Err(Error::DuplicateHash(0x70))
    ));
}

#[test]
fn unknown_type_rejected() {
    let data = file(&[entry(0x71, 0x1234, 0, &[])]);
    assert!(matches!(
        PropertyFile::parse(&data),
        Err(Error::UnknownPropertyType(0x1234, 0x71))
    ));
}

#[test]
fn truncated_payload_rejected() {
    let data = file(&[entry(0x72, 13, 0, &[0x3F, 0x80])]); // float needs 4 bytes
    assert!(matches!(
        PropertyFile::parse(&data),
        Err(Error::Truncated { .. })
    ));
}

#[test]
fn parse_encode_parse_preserves_values_and_encoding_metadata() {
    let mut array = be32(2).to_vec();
    array.extend_from_slice(&be32(4));
    array.extend_from_slice(&be32(7));
    array.extend_from_slice(&be32(9));
    let mut transform = be16(15).to_vec();
    push_f32(&mut transform, 2.5);
    for value in [1.0; 12] {
        push_f32(&mut transform, value);
    }
    let data = file(&[
        entry(0x20, 10, 0, &be32(42)),
        entry(0x10, 10, 0x8090, &array),
        entry(0x30, 56, 0, &transform),
    ]);
    let first = PropertyFile::parse(&data).unwrap();
    assert_eq!(first.get(0x10).unwrap().encoding.flags, 0x8090);
    assert_eq!(first.get(0x10).unwrap().encoding.array_item_size, Some(4));
    let encoded = first.encode_canonical().unwrap();
    let second = PropertyFile::parse(&encoded).unwrap();
    for hash in [0x10, 0x20, 0x30] {
        assert_eq!(second.get(hash), first.get(hash));
    }
}

#[test]
fn canonical_encoder_rejects_incompatible_array_item_size() {
    let property = Property {
        hash: 1,
        prop_type: PropType::UInt32,
        kind: Kind::Array(vec![Value::UInt32(1)]),
        encoding: PropertyEncoding {
            flags: 0x30,
            array_item_size: Some(8),
        },
    };
    let file = PropertyFile {
        values: vec![property],
        claimed_count: 1,
    };
    assert!(matches!(
        file.encode_canonical(),
        Err(Error::InvalidArrayItemSize { .. })
    ));
}

#[test]
fn canonical_encoder_rejects_incompatible_value_type() {
    let property = Property {
        hash: 1,
        prop_type: PropType::UInt32,
        kind: Kind::Scalar(Value::Int32(1)),
        encoding: PropertyEncoding::default(),
    };
    let file = PropertyFile {
        values: vec![property],
        claimed_count: 1,
    };
    assert!(matches!(
        file.encode_canonical(),
        Err(Error::ValueTypeMismatch { .. })
    ));
}
#[test]
fn dump_display_sorted_by_hash() {
    let data = file(&[
        entry(0x02, 10, 0, &be32(1)),
        entry(0x01, 13, 0, &{
            let mut p = Vec::new();
            push_f32(&mut p, 0.5);
            p
        }),
    ]);
    let pf = PropertyFile::parse(&data).unwrap();
    let dump = format!("{pf}");
    let lines: Vec<&str> = dump.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("0x00000001"));
    assert!(lines[1].starts_with("0x00000002"));
}

// ---- combine 聚合 ----

mod combine_tests {
    use super::*;
    use crate::{MODEL_DETAILS_HASH, combine_assets};
    use dbpf::ResourceId;

    fn rid(instance: u32, group: u32) -> ResourceId {
        ResourceId {
            type_id: 0x00B1_B104,
            group,
            instance,
        }
    }

    fn prop_file(props: Vec<Property>) -> PropertyFile {
        let claimed_count = props.len() as u32;
        PropertyFile {
            values: props,
            claimed_count,
        }
    }

    fn key_prop(hash: u32, target_instance: u32) -> Property {
        Property {
            hash,
            prop_type: PropType::Key,
            kind: Kind::Scalar(Value::Key(Key {
                instance: target_instance,
                type_id: 0x00B1_B104,
                group: 0,
            })),
            encoding: PropertyEncoding::default(),
        }
    }

    fn simple_prop(hash: u32, value: u32) -> Property {
        Property {
            hash,
            prop_type: PropType::UInt32,
            kind: Kind::Scalar(Value::UInt32(value)),
            encoding: PropertyEncoding::default(),
        }
    }

    #[test]
    fn combines_same_instance_and_model_details() {
        // A(0x1111, model) + B(0x1111, gameplay) 同实例；C(0x2222, catalog)
        // 通过 Model Details 指向 0x1111；D、E 独立；F 的其他 hash 引用不合并。
        let model_details = key_prop(MODEL_DETAILS_HASH, 0x1111);
        let other_ref = key_prop(0x0DB9_FC63, 0x1111); // Parent-Menu 类引用，必须忽略

        let entries = [
            (
                rid(0x1111, 0x40E1_C000),
                prop_file(vec![simple_prop(0x01, 1)]),
            ),
            (
                rid(0x1111, 0x40E0_C000),
                prop_file(vec![simple_prop(0x02, 2)]),
            ),
            (
                rid(0x2222, 0x0987_8A01),
                prop_file(vec![model_details, simple_prop(0x03, 3)]),
            ),
            (rid(0x3333, 0), prop_file(vec![simple_prop(0x04, 4)])),
            (
                rid(0x4444, 0),
                prop_file(vec![key_prop(MODEL_DETAILS_HASH, 0x9999)]),
            ), // 悬空引用
            (rid(0x5555, 0), prop_file(vec![other_ref])),
        ];

        let names = std::iter::once((0x1111_u32, "Central Station".to_string())).collect();
        let groups = combine_assets(entries.iter().map(|(id, f)| (*id, f)), Some(&names));

        assert_eq!(
            groups.len(),
            4,
            "6 entries, A+B+C merge -> 4 groups; got {groups:?}"
        );

        // 排序后前三组是 group=0 的单成员（D、E、F），合并组按其最小成员组号排最后
        assert!(groups[0..3].iter().all(|g| g.members.len() == 1));
        assert!(groups.iter().all(|g| g.members.contains(&rid(0x4444, 0))
            == (g.members.len() == 1 && g.members[0].instance == 0x4444)));

        let combined = groups
            .iter()
            .find(|g| g.members.len() == 3)
            .expect("A+B+C should merge into one group");
        assert_eq!(combined.name.as_deref(), Some("Central Station"));
        assert!(
            combined.members.contains(&rid(0x2222, 0x0987_8A01)),
            "catalog prop joins via Model Details"
        );
        // 悬空引用组无名称
        assert!(
            groups
                .iter()
                .filter(|g| g.members.contains(&rid(0x4444, 0)))
                .all(|g| g.name.is_none())
        );
    }

    #[test]
    fn model_details_in_array_also_unions() {
        let array_prop = Property {
            hash: MODEL_DETAILS_HASH,
            prop_type: PropType::Key,
            kind: Kind::Array(vec![
                Value::Key(Key {
                    instance: 0x7777,
                    type_id: 0x00B1_B104,
                    group: 0,
                }),
                Value::Key(Key {
                    instance: 0x8888,
                    type_id: 0x00B1_B104,
                    group: 0,
                }),
            ]),
            encoding: PropertyEncoding::default(),
        };
        let entries = [
            (rid(0x7777, 0), prop_file(vec![simple_prop(1, 1)])),
            (rid(0x8888, 0), prop_file(vec![simple_prop(2, 2)])),
            (rid(0x6666, 0), prop_file(vec![array_prop])),
        ];
        let groups = combine_assets(entries.iter().map(|(id, f)| (*id, f)), None);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].members.len(), 3);
    }
}

// ---- locale ----

mod locale_tests {
    use super::*;
    use crate::locale::{
        Locale, MODEL_RESOURCE_TYPE, NAME_PROPERTY_HASHES, collect_name_map, parse_string_table,
    };
    use dbpf::ResourceId;

    fn rid(instance: u32) -> ResourceId {
        ResourceId {
            type_id: 0x00B1_B104,
            group: 0,
            instance,
        }
    }

    #[test]
    fn parses_locale_json_table() {
        let mut data = vec![0xEF, 0xBB, 0xBF];
        data.extend_from_slice(br#"{"0x00000001": "Maxis Manor", "//": "names", "0x2": "Foo"}"#);

        let table = parse_string_table(&data).unwrap();
        assert_eq!(table.get(&1).map(String::as_str), Some("Maxis Manor"));
        assert_eq!(table.get(&2).map(String::as_str), Some("Foo"));
        assert_eq!(table.len(), 2, "comment entry must be skipped");
    }

    #[test]
    fn collects_names_and_propagates_to_models() {
        let locale = Locale::from_resources(vec![(0xA000, {
            let mut d = vec![0xEF, 0xBB, 0xBF];
            d.extend_from_slice(br#"{"0x0100": "Airship Hangar"}"#);
            d
        })])
        .unwrap();
        assert_eq!(locale.table_count(), 1);
        assert_eq!(locale.get(0xA000, 0x0100), Some("Airship Hangar"));

        // 名称属性：Array[0] = Text{table 0xA000, id 0x0100}，同时 Key 引用模型 0x2F4E681B/0xFEED
        let name_hash = NAME_PROPERTY_HASHES[0];
        let name_prop = Property {
            hash: name_hash,
            prop_type: PropType::Text,
            kind: Kind::Array(vec![
                Value::Text(crate::Text {
                    table_id: 0xA000,
                    instance_id: 0x0100,
                }),
                Value::Key(Key {
                    instance: 0xFEED,
                    type_id: MODEL_RESOURCE_TYPE,
                    group: 0,
                }),
            ]),
            encoding: PropertyEncoding::default(),
        };
        let entries = [(
            rid(0x1234),
            PropertyFile {
                values: vec![name_prop],
                claimed_count: 1,
            },
        )];

        let names = collect_name_map(entries.iter().map(|(id, f)| (*id, f)), &locale);
        assert_eq!(
            names.get(&0x1234).map(String::as_str),
            Some("Airship Hangar")
        );
        assert_eq!(
            names.get(&0xFEED).map(String::as_str),
            Some("Airship Hangar"),
            "name propagates to referenced model"
        );
    }
}

#[test]
fn locale_items_round_trip_preserves_comments_and_keys() {
    use crate::locale::{parse_locale_items, serialize_locale_items};
    let original = [
        0xEF, 0xBB, 0xBF, b'{', b'"', b'/', b'/', b'"', b':', b'"', b'h', b'i', b'"', b',',
        b'"', b'0', b'x', b'0', b'0', b'0', b'0', b'0', b'0', b'0', b'1', b'"', b':',
        b'"', 0xE4, 0xBD, 0xA0, 0xE5, 0xA5, 0xBD, b'"', b'}',
    ];
    let items = parse_locale_items(&original).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, None);
    assert_eq!(items[1].id, Some(1));
    assert_eq!(items[1].text, "你好");
    let bytes = serialize_locale_items(&items).unwrap();
    // 前缀保留、注释保留、规范键保留
    assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF]);
    let reparsed = parse_locale_items(&bytes).unwrap();
    assert_eq!(reparsed, items);
}

#[test]
fn serialize_locale_items_rejects_duplicates() {
    use crate::locale::{serialize_locale_items, LocaleItem};
    let items = vec![
        LocaleItem { key: "0x00000001".into(), id: Some(1), text: "a".into() },
        LocaleItem { key: "0x1".into(), id: Some(1), text: "b".into() },
    ];
    assert!(serialize_locale_items(&items).is_err());
}
