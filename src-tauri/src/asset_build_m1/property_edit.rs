//! Shared PropertyFile edit helpers (extracted from `menu.rs` so the behavior
//! processor and the menu processor mutate properties with identical
//! encoding semantics: upsert-by-hash, declared item sizes, last-write dedup).

use sc_properties::{
    Kind, PropType, Property, PropertyEncoding, PropertyFile, TEXT_ARRAY_ITEM_SIZE, Value,
};

/// Upsert a scalar property by hash (append when missing).
pub(crate) fn set_scalar(file: &mut PropertyFile, hash: u32, prop_type: PropType, value: Value) {
    match file
        .values
        .iter_mut()
        .find(|property| property.hash == hash)
    {
        Some(property) => {
            property.kind = Kind::Scalar(value);
            property.prop_type = prop_type;
        }
        None => file.values.push(Property {
            hash,
            prop_type,
            kind: Kind::Scalar(value),
            encoding: PropertyEncoding::default(),
        }),
    }
}

/// Upsert an array property by hash, deriving the declared item size from the
/// property type (Keys 12 B, Text 520 B, 32-bit scalars 4 B).
pub(crate) fn set_array(
    file: &mut PropertyFile,
    hash: u32,
    prop_type: PropType,
    values: Vec<Value>,
) {
    let item_size = match prop_type {
        PropType::Key => Some(12),
        PropType::Text => Some(TEXT_ARRAY_ITEM_SIZE),
        PropType::Int32 | PropType::UInt32 | PropType::Float => Some(4),
        // vanilla 全量标定（scan_strides）每个类型恰好一个合法步长：
        // BoundingBox 24 / Vector2 8 / Vector3·ColorRgb 12。缺声明会被游戏
        // 按步长 0 整文件丢弃（layout-contract 会拦），故必须显式给出。
        PropType::BoundingBox => Some(24),
        PropType::Vector2 => Some(8),
        PropType::Vector3 | PropType::ColorRgb => Some(12),
        _ => None,
    };
    match file
        .values
        .iter_mut()
        .find(|property| property.hash == hash)
    {
        Some(property) => {
            property.kind = Kind::Array(values);
            property.prop_type = prop_type;
            property.encoding.array_item_size = item_size;
        }
        None => file.values.push(Property {
            hash,
            prop_type,
            kind: Kind::Array(values),
            encoding: PropertyEncoding {
                flags: 0,
                array_item_size: item_size,
            },
        }),
    }
}

/// `set_array` + explicit encoding flags (e.g. `0x809C` key arrays, `0x801C`
/// text arrays).
pub(crate) fn set_array_with_flags(
    file: &mut PropertyFile,
    hash: u32,
    prop_type: PropType,
    values: Vec<Value>,
    flags: u16,
) {
    set_array(file, hash, prop_type, values);
    if let Some(property) = file
        .values
        .iter_mut()
        .find(|property| property.hash == hash)
    {
        property.encoding.flags = flags;
    }
}

/// Canonical encode with last-write-wins dedup: the game rejects a property
/// list containing the same hash twice, and a processor may legitimately set a
/// field the donor already carried.
pub(crate) fn encode(file: &PropertyFile) -> Result<Vec<u8>, String> {
    let mut values: Vec<Property> = Vec::with_capacity(file.values.len());
    for property in &file.values {
        if let Some(existing) = values.iter_mut().find(|item| item.hash == property.hash) {
            *existing = property.clone();
        } else {
            values.push(property.clone());
        }
    }
    let mut deduped = file.clone();
    deduped.values = values;
    deduped
        .encode_canonical()
        .map_err(|error| format!("property encode: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_properties::Key;

    #[test]
    fn set_scalar_upserts_by_hash() {
        let mut file = PropertyFile::default();
        set_scalar(&mut file, 0x10, PropType::Float, Value::Float(1.0));
        set_scalar(&mut file, 0x10, PropType::Float, Value::Float(2.5));
        assert_eq!(file.values.len(), 1);
        assert_eq!(file.values[0].kind, Kind::Scalar(Value::Float(2.5)));
    }

    #[test]
    fn set_array_declares_key_item_size() {
        let mut file = PropertyFile::default();
        set_array(
            &mut file,
            0x20,
            PropType::Key,
            vec![Value::Key(Key {
                type_id: 0,
                group: 0,
                instance: 1,
            })],
        );
        assert_eq!(file.values[0].encoding.array_item_size, Some(12));
    }

    #[test]
    fn encode_dedupes_repeated_hashes() {
        let mut file = PropertyFile::default();
        set_scalar(&mut file, 0x30, PropType::Float, Value::Float(1.0));
        set_scalar(&mut file, 0x30, PropType::Float, Value::Float(9.0));
        let bytes = encode(&file).expect("encode");
        let parsed = PropertyFile::parse(&bytes).expect("reparse");
        assert_eq!(parsed.values.len(), 1);
    }
}
