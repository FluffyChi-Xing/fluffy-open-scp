//! Add the retail demolish bundle to the retained homeless-tent Unit.
//!
//! The tent is a valid ploppable Unit, but its five-property Unit resource has
//! no `0x0A0209A3` bundle reference.  Bulldozer calls the native player rule
//! `0x0C614161` before queuing demolition; the retail script bundle
//! `40E0C700:2A2459B7` supplies that rule and its event mapping.  This probe
//! overlays only the tent Unit property and leaves the menu and all other
//! assets untouched.
//!
//! ```text
//! cargo run -p probes --release --example tent_bulldozer_overlay -- \
//!   <SimCity_Game.package> <out.package>
//! ```

use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::{Key, Kind, PropType, Property, PropertyEncoding, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const UNIT_GROUP: u32 = 0x40E1_C000;
const TENT_UNIT: u32 = 0xEB41_5C8F;
const BUNDLE_LIST_HASH: u32 = 0x0A02_09A3;
const DEMOLISH_BUNDLE: u32 = 0x2A24_59B7;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: tent_bulldozer_overlay <SimCity_Game.package> <out.package>");
        std::process::exit(2);
    }

    let base = Package::open(&args[0]).expect("open SimCity_Game.package");
    let id = ResourceId {
        type_id: PROPERTY_TYPE,
        group: UNIT_GROUP,
        instance: TENT_UNIT,
    };
    let entry = base
        .entries()
        .iter()
        .find(|entry| entry.id == id)
        .expect("homeless tent Unit property not found");
    let original = base.read(entry).expect("read tent Unit property");
    let mut file = PropertyFile::parse(&original).expect("parse tent Unit property");
    assert!(
        file.get(BUNDLE_LIST_HASH).is_none(),
        "tent already contains a bundle list; do not overwrite it"
    );

    file.values.push(Property {
        hash: BUNDLE_LIST_HASH,
        prop_type: PropType::Key,
        kind: Kind::Array(vec![Value::Key(Key {
            instance: DEMOLISH_BUNDLE,
            type_id: 0,
            group: 0,
        })]),
        encoding: PropertyEncoding {
            flags: 0x30,
            array_item_size: Some(12),
        },
    });
    let patched = file.encode_canonical().expect("encode patched tent Unit");
    let check = PropertyFile::parse(&patched).expect("parse patched tent Unit");
    let property = check.get(BUNDLE_LIST_HASH).expect("bundle list missing");
    assert!(matches!(
        &property.kind,
        Kind::Array(values)
            if values.len() == 1
                && matches!(&values[0], Value::Key(key) if key.instance == DEMOLISH_BUNDLE)
    ));

    if let Some(parent) = std::path::Path::new(&args[1]).parent() {
        std::fs::create_dir_all(parent).expect("create output directory");
    }
    dbpf::write_uncompressed_overlay_to_path(&args[1], &[OverlayEntry::new(id, patched.clone())])
        .expect("write overlay");
    println!(
        "wrote {id} {} -> {} bytes; bundle 0x{DEMOLISH_BUNDLE:08X} added; verified=true",
        original.len(),
        patched.len()
    );
}
