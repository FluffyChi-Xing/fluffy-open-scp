//! Build a one-region Terrain Authoring environment overlay.
//!
//! This changes only `0x0E276F5D` on the selected `environment` property. It
//! is deliberately separate from the terrain brush overlay: the field enables
//! the `regionedit` camera and renderer state, while the native retail build
//! still needs a caller for `SetTerrainAuthoringToolActive(true)` to expose the
//! bottom authoring palette.
//!
//! ```text
//! cargo run -p probes --release --example terrain_authoring_overlay -- \
//!   <SimCity_RegionTerrain*.package> <out.package> <region_group_hex>
//! ```

use std::path::PathBuf;

use dbpf::{OverlayEntry, Package};
use sc_properties::{debug_tools, Kind, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const ENVIRONMENT_INSTANCE: u32 = 0x494F_8678;
const ENV_ENABLE: u32 = 0x0E27_6F5D;
const ENV_REGION: u32 = 0xC194_9C4D;
const REGION_INSTANCE: u32 = 0x51E7_A18D;

fn parse_hex(value: &str) -> u32 {
    u32::from_str_radix(value.trim_start_matches("0x"), 16).expect("invalid hex")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        eprintln!(
            "usage: terrain_authoring_overlay <region-terrain.package> <out.package> <region-group-hex>"
        );
        std::process::exit(2);
    }
    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let region_group = parse_hex(&args[2]);
    let package = Package::open(&input).expect("open input package");

    let entry = package
        .entries()
        .iter()
        .find(|entry| {
            entry.id.type_id == PROPERTY_TYPE
                && entry.id.group == region_group
                && entry.id.instance == ENVIRONMENT_INSTANCE
        })
        .unwrap_or_else(|| {
            panic!(
                "environment property {:08X}:{:08X}:{:08X} not found",
                PROPERTY_TYPE, region_group, ENVIRONMENT_INSTANCE
            )
        });
    let data = package.read(entry).expect("read environment property");
    let file = PropertyFile::parse(&data).expect("parse environment property");
    let enabled = match file.get(ENV_ENABLE).map(|property| &property.kind) {
        Some(Kind::Scalar(Value::Bool(value))) => *value,
        _ => panic!("environment property has no scalar enable bool"),
    };
    let region = match file.get(ENV_REGION).map(|property| &property.kind) {
        Some(Kind::Scalar(Value::Key(key))) => *key,
        _ => panic!("environment property has no region key"),
    };
    assert_eq!(region.group, region_group, "region key points to another group");
    assert_eq!(region.instance, REGION_INSTANCE, "unexpected region key instance");
    if enabled {
        eprintln!("environment is already enabled; refusing a no-op overlay");
        std::process::exit(3);
    }

    let patched = debug_tools::patch_property_bool(&data, ENV_ENABLE, false, true)
        .expect("patch environment enable bool");
    let patched_file = PropertyFile::parse(&patched).expect("parse patched property");
    assert!(matches!(
        patched_file.get(ENV_ENABLE).map(|property| &property.kind),
        Some(Kind::Scalar(Value::Bool(true)))
    ));

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).expect("create output directory");
    }
    let bytes = dbpf::write_uncompressed_overlay(&[OverlayEntry::new(entry.id, patched)])
        .expect("encode overlay");
    std::fs::write(&output, bytes).expect("write overlay");
    println!(
        "enabled region {:08X}; input={} output={} (one property, original={}B)",
        region_group,
        input.display(),
        output.display(),
        data.len()
    );
    println!(
        "expected first observation: regionedit camera + DOF/motionBlur disabled; bottom Terrain Authoring palette requires a separate native active-state caller"
    );
}
