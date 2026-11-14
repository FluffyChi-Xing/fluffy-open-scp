//! Read-only source audit and isolated A/B/C packages; never installs into the game.
//! Args: copied-menu.package scripts.package output-directory
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use sc_properties::{Kind, PropertyFile, Value};
use std::path::Path;

fn read(pkg: &Package, id: ResourceId) -> Vec<u8> {
    pkg.read(
        pkg.entries()
            .iter()
            .find(|e| e.id == id)
            .expect("source TGI"),
    )
    .expect("read")
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(
        args.len(),
        3,
        "copied-menu.package scripts.package output-directory"
    );
    let source = Package::open(&args[0]).expect("menu package");
    let scripts = Package::open(&args[1]).expect("script package");
    let menu_id = ResourceId {
        type_id: 0x00b1b104,
        group: 0x09878a01,
        instance: 0xf8196d0d,
    };
    let unit_id = ResourceId {
        type_id: 0x00b1b104,
        group: 0x40e0c000,
        instance: 0xe917279c,
    };
    let menu = read(&source, menu_id);
    let menu_props = PropertyFile::parse(&menu).unwrap();
    assert_eq!(
        menu_props
            .get(0x0975695f)
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .instance,
        unit_id.instance
    );
    assert_eq!(
        menu_props
            .get(0x0db9fc63)
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .instance,
        0xac12c904
    );
    let raw = read(&scripts, unit_id);
    let mut file = PropertyFile::parse(&raw).unwrap();
    assert_eq!(
        file.encode_canonical().unwrap(),
        raw,
        "unchanged roundtrip must be byte exact"
    );
    let cost = file
        .values
        .iter_mut()
        .find(|p| p.hash == 0x0a8a41da)
        .unwrap();
    assert!(matches!(cost.kind, Kind::Scalar(Value::Float(20000.0))));
    cost.kind = Kind::Scalar(Value::Float(20.0));
    let changed = file.encode_canonical().unwrap();
    assert_eq!(raw.len(), changed.len());
    let differences: Vec<_> = raw
        .iter()
        .zip(&changed)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| format!("0x{i:04X}: {a:02X}->{b:02X}"))
        .collect();
    assert!(differences.len() <= 4);
    let reread = PropertyFile::parse(&changed).unwrap();
    for prop in PropertyFile::parse(&raw).unwrap().values {
        if prop.hash != 0x0a8a41da {
            assert_eq!(reread.get(prop.hash), Some(&prop));
        }
    }
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    for (name, unit) in [
        ("A-menu-only", None),
        ("B-unit-unchanged", Some(raw.clone())),
        ("C-price-20", Some(changed)),
    ] {
        let mut entries = vec![OverlayEntry::new(menu_id, menu.clone())];
        if let Some(data) = unit {
            entries.push(OverlayEntry::new(unit_id, data));
        }
        let path = out.join(format!("{name}.package"));
        assert!(!path.exists(), "do not overwrite previous evidence");
        std::fs::write(&path, write_uncompressed_overlay(&entries).unwrap()).unwrap();
        let check = Package::open(&path).unwrap();
        for entry in &entries {
            assert_eq!(read(&check, entry.id), entry.data);
        }
        println!(
            "{}: {} resources, DBPF reread exact",
            path.display(),
            entries.len()
        );
    }
    println!(
        "Unit {} bytes; unchanged canonical roundtrip exact; only price differs: {}",
        raw.len(),
        differences.join(", ")
    );
}
