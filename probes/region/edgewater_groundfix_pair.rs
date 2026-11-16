//! Package two R1 menu builds with a controlled floating-parcel difference.
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use sc_properties::locale::{parse_locale_items, serialize_locale_items};
use sc_properties::{Kind, PropType, PropertyFile, Value};
use std::collections::{HashMap, HashSet};
use std::{error::Error, fs};

const FLOATING: u32 = 0x0E2E4B86;
const LOT: u32 = 0x40E1C000;
const MENU: u32 = 0x09878A01;
const SOURCE_LOCALE: ResourceId = ResourceId {
    type_id: 0x0A98EAF0,
    group: 0x02FABF01,
    instance: 0xBF82F7CE,
};
// FNV-1("openscp/edgewater/groundfix/ab"). The installed UI overlay owns rows
// in SOURCE_LOCALE, so the test menus use a dedicated table.
const PAIR_LOCALE: ResourceId = ResourceId {
    instance: 0x64056C97,
    ..SOURCE_LOCALE
};

fn prop_id(group: u32, instance: u32) -> ResourceId {
    ResourceId {
        type_id: 0x00B1B104,
        group,
        instance,
    }
}

fn read(package: &Package, id: ResourceId) -> Vec<u8> {
    package
        .read(package.entry(id).expect("missing resource"))
        .unwrap()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        args.len() >= 5,
        "usage: <control> <floating> <orion> <output> <reference packages...>"
    );
    let reference = Package::open(&args[2])?;
    let donor = PropertyFile::parse(&read(&reference, prop_id(LOT, 0x18D36A29)))?;
    let marker = donor.values.iter().find(|p| p.hash == FLOATING).unwrap();
    assert_eq!(marker.prop_type, PropType::Bool);
    assert_eq!(marker.scalar(), Some(&Value::Bool(true)));

    let mut resources = HashMap::new();
    let mut names = Vec::new();
    let mut control_lot = None;
    let mut control_texture = None;
    for (index, (instance, floating)) in [(0xFB5AA2CE, false), (0x990FA812, true)]
        .into_iter()
        .enumerate()
    {
        let source = Package::open(&args[index])?;
        let mut rows = parse_locale_items(&read(&source, SOURCE_LOCALE))?;
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            [Some(1), Some(2)]
        );
        let title = if floating {
            "EdgeWaterBay 地坪测试 Floating"
        } else {
            "EdgeWaterBay 地坪测试 Control"
        };
        assert_eq!(rows[0].text, title);
        let first_row = 1 + 2 * index as u32;
        rows[0].id = Some(first_row);
        rows[1].id = Some(first_row + 1);
        names.extend(rows);

        let lot_id = prop_id(LOT, instance);
        let menu_id = prop_id(MENU, instance);
        assert!(source.entry(lot_id).is_some() && source.entry(menu_id).is_some());
        for entry in source
            .entries()
            .iter()
            .filter(|entry| entry.id != SOURCE_LOCALE)
        {
            let mut data = source.read(entry)?;
            if entry.id == lot_id {
                let mut lot = PropertyFile::parse(&data)?;
                assert_eq!(lot.encode_canonical()?, data);
                assert!(lot.values.iter().all(|p| p.hash != FLOATING));
                let texture_property = lot.values.iter().find(|p| p.hash == 0x0CCB7FD5).unwrap();
                let Some(Value::Key(texture)) = texture_property.scalar() else {
                    panic!("expected lot-ground texture key");
                };
                let texture_data = read(
                    &source,
                    ResourceId {
                        type_id: texture.type_id,
                        group: texture.group,
                        instance: texture.instance,
                    },
                );
                let comparable: Vec<_> = lot
                    .values
                    .iter()
                    .filter(|p| p.hash != texture_property.hash)
                    .cloned()
                    .collect();
                if floating {
                    assert_eq!(control_lot.as_ref(), Some(&comparable));
                    assert_eq!(control_texture.as_ref(), Some(&texture_data));
                } else {
                    control_lot = Some(comparable);
                    control_texture = Some(texture_data);
                }
                let mut flag = marker.clone();
                flag.kind = Kind::Scalar(Value::Bool(floating));
                lot.values.push(flag);
                data = lot.encode_canonical()?;
                assert_eq!(PropertyFile::parse(&data)?.values, lot.values);
            }
            if entry.id == menu_id {
                let mut menu = PropertyFile::parse(&data)?;
                for (hash, row) in [(0x0A09F5FA, first_row), (0x0A09F5FB, first_row + 1)] {
                    let property = menu.values.iter_mut().find(|p| p.hash == hash).unwrap();
                    let Kind::Array(values) = &mut property.kind else {
                        panic!("expected text array")
                    };
                    let [Value::Text(text)] = values.as_mut_slice() else {
                        panic!("expected one text reference")
                    };
                    assert_eq!(text.table_id, SOURCE_LOCALE.instance);
                    text.table_id = PAIR_LOCALE.instance;
                    text.instance_id = row;
                }
                data = menu.encode_canonical()?;
                assert_eq!(PropertyFile::parse(&data)?.values, menu.values);
            }
            if let Some(previous) = resources.insert(entry.id, data.clone()) {
                assert_eq!(
                    previous, data,
                    "conflicting generated resource {}",
                    entry.id
                );
            }
        }
    }
    assert!(
        resources
            .insert(PAIR_LOCALE, serialize_locale_items(&names)?)
            .is_none()
    );

    let mut duplicates = HashSet::new();
    for path in &args[4..] {
        let package = Package::open(path)?;
        for entry in package.entries() {
            if let Some(generated) = resources.get(&entry.id) {
                assert_eq!(
                    &package.read(entry)?,
                    generated,
                    "test package would overwrite {} in {path}",
                    entry.id
                );
                duplicates.insert(entry.id);
            }
        }
    }
    let entries: Vec<_> = resources
        .into_iter()
        .filter(|(id, _)| !duplicates.contains(id))
        .map(|(id, data)| OverlayEntry::new(id, data))
        .collect();
    fs::write(&args[3], write_uncompressed_overlay(&entries)?)?;
    let output = Package::open(&args[3])?;
    assert_eq!(output.entries().len(), entries.len());
    for entry in entries {
        assert_eq!(read(&output, entry.id), entry.data);
    }
    println!(
        "{} resources; {} identical reference resources omitted",
        output.entries().len(),
        duplicates.len()
    );
    Ok(())
}
