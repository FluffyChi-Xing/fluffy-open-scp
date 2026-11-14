//! One-off diagnostic: keep the known-good NightHouse identity/lot/menu and
//! replace only its four LOD model payloads with our compiled geometry.
//!
//! cargo run -p fluffy-open-scp --example nh_geometry_probe -- \
//!   <nighthouse.package> <our-model.rw4> <out.package>

use dbpf::{OverlayEntry, Package, write_uncompressed_overlay};
use std::{env, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: nh_geometry_probe <source.package> <model.rw4> <out.package>".into());
    }
    let source = Package::open(&args[0])?;
    let model = fs::read(&args[1])?;
    let ids = [0x06C5470C, 0x06C5470D, 0x06C5470E, 0x06C5470F];
    let mut entries = Vec::new();
    for entry in source.entries() {
        let mut bytes = source.read(entry)?;
        if ids.contains(&entry.id.instance) && entry.id.type_id == 0x2F4E681B {
            bytes = model.clone();
        }
        if entry.id.type_id == 0x0A98EAF0 && entry.id.instance == 0x78BAB639 {
            let text = String::from_utf8_lossy(&bytes)
                .replace("SC5 Night House 0.5", "OpenSCP NH Geometry Test")
                .replace(
                    "Native interior controller enabled. Day/night room switching with verified recessed windows.",
                    "Diagnostic: NightHouse identity + OpenSCP compiled geometry.",
                );
            bytes = text.into_bytes();
        }
        entries.push(OverlayEntry::new(entry.id, bytes));
    }
    fs::write(&args[2], write_uncompressed_overlay(&entries)?)?;
    println!("wrote {}", args[2]);
    Ok(())
}
