//! Offline proof: move authored scene columns from Lot into a cloned 41F0C000
//! Building Asset Property and link Lot <-> Asset with 00B2CCCB.
use sc_properties::{Key, Kind, PropertyFile, PropType, Value};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 8 {
        return Err("usage: base_lot scene_lot ref_asset out_asset out_lot lot_inst asset_inst".into());
    }
    let read = |path: &str| std::fs::read(path).map_err(|error| format!("{path}: {error}"));
    let mut base_lot = PropertyFile::parse(&read(&args[1])?).map_err(|e| e.to_string())?;
    let scene_lot = PropertyFile::parse(&read(&args[2])?).map_err(|e| e.to_string())?;
    let mut asset = PropertyFile::parse(&read(&args[3])?).map_err(|e| e.to_string())?;
    let lot_instance = u32::from_str_radix(args[6].trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let asset_instance = u32::from_str_radix(args[7].trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;

    let scene_prefix = |hash: u32| {
        matches!(
            hash >> 16,
            0x0CAA | 0x0C12 | 0x0E1B | 0x0E71 | 0x0F0E
        )
    };
    asset.values.retain(|property| !scene_prefix(property.hash));
    for property in &scene_lot.values {
        if scene_prefix(property.hash) {
            asset.values.push(property.clone());
        }
    }
    for hash in [0x00F9_EFBA, 0x00F9_EFBB, 0x00F9_EFBC, 0x00F9_EFBD, 0x00F9_EFBE] {
        if let Some(source) = base_lot.values.iter().find(|property| property.hash == hash) {
            if let Some(target) = asset.values.iter_mut().find(|property| property.hash == hash) {
                *target = source.clone();
            } else {
                asset.values.push(source.clone());
            }
        }
    }
    let set_key = |file: &mut PropertyFile, hash: u32, key: Key| {
        if let Some(property) = file.values.iter_mut().find(|property| property.hash == hash) {
            property.prop_type = PropType::Key;
            property.kind = Kind::Scalar(Value::Key(key));
        }
    };
    set_key(
        &mut asset,
        0x00B2_CCCB,
        Key { instance: lot_instance, type_id: 0xB1B104, group: 0x40E1C000 },
    );
    set_key(
        &mut base_lot,
        0x00B2_CCCB,
        Key { instance: asset_instance, type_id: 0xB1B104, group: 0x41F0C000 },
    );
    let self_key = Key { instance: asset_instance, type_id: 0xB1B104, group: 0x41F0C000 };
    if let Some(property) = asset.values.iter_mut().find(|property| property.hash == 0x0CA5_CF4A) {
        property.prop_type = PropType::Key;
        property.encoding = sc_properties::PropertyEncoding { flags: 0, array_item_size: Some(12) };
        property.kind = Kind::Array(vec![Value::Key(self_key)]);
    } else {
        asset.values.push(sc_properties::Property {
            hash: 0x0CA5_CF4A,
            prop_type: PropType::Key,
            kind: Kind::Array(vec![Value::Key(self_key)]),
            encoding: sc_properties::PropertyEncoding { flags: 0, array_item_size: Some(12) },
        });
    }
    std::fs::write(&args[4], asset.encode_canonical().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::write(&args[5], base_lot.encode_canonical().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    println!("asset={} lot={}", asset.values.len(), base_lot.values.len());
    Ok(())
}
