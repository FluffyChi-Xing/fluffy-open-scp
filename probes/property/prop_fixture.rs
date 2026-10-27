//! 冒烟工具：为前端 property-writer 测试生成 encode_canonical 十六进制 fixture。
use sc_properties::{Key, Kind, PropType, Property, PropertyEncoding, PropertyFile, Text, Value};

fn main() {
    // 一组固定字段：int32、uint32、key、text、bool、以及 text[]
    let mut values = vec![
        Property { hash: 0x0DC1E3E0, prop_type: PropType::Int32, kind: Kind::Scalar(Value::Int32(300)), encoding: PropertyEncoding { flags: 0, array_item_size: None } },
        Property { hash: 0x0A09F5FA, prop_type: PropType::Text, kind: Kind::Scalar(Value::Text(Text { table_id: 0x6C969DEE, instance_id: 0x64EC016B })), encoding: PropertyEncoding { flags: 0, array_item_size: None } },
        Property { hash: 0x0977AA8F, prop_type: PropType::Key, kind: Kind::Scalar(Value::Key(Key { instance: 0x83F06C63, type_id: 0x2F7D0004, group: 0x40E02400 })), encoding: PropertyEncoding { flags: 0, array_item_size: None } },
        Property { hash: 0x0EB1FC05, prop_type: PropType::Text, kind: sc_properties::Kind::Array(vec![Value::Text(Text { table_id: 0x50AA0BEA, instance_id: 0x0EB1FE41 }), Value::Text(Text { table_id: 0x50AA0BEA, instance_id: 0x0EB1FE44 })]), encoding: PropertyEncoding { flags: 0x30, array_item_size: Some(8) } },
        Property { hash: 0x0F1A181D, prop_type: PropType::Bool, kind: Kind::Scalar(Value::Bool(true)), encoding: PropertyEncoding { flags: 0, array_item_size: None } },
    ];
    values.sort_by_key(|p| p.hash);
    let file = PropertyFile { values, claimed_count: 5 };
    let bytes = file.encode_canonical().unwrap();
    println!("{}", bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>());
}
