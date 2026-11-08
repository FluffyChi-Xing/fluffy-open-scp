//! Ground-lot input trimming for the SC5 Asset Packager route.
//!
//! The compile route accepts exactly: LOT_TYPES numerics plus the ground
//! raster reference 0x0CCB7FD5 (and 0x0A0209A4 in the latest build). Retail
//! lots carry extra fields (0x0CCB7FD3/4, 0x0D0082F1, ...) that get
//! rejected, so this example drops everything outside the whitelist.
//!
//! 道具/灯/行为等属性归我方管线；这里只做 ground 输入的模型侧准备。
//!
//! Usage: cargo run -p fluffy-open-scp --example trim_ground_lot -- \
//!   <ground-in.package> <ground-out.package>
use dbpf::{OverlayEntry, Package, write_uncompressed_overlay};
use sc_properties::PropertyFile;
use std::path::Path;

const PARCEL_FIELDS: [u32; 5] = [
    0x0CCB_7FC8, // size
    0x0CCB_7FC9, // offset
    0x0CCB_7FD0, // rotation
    0x0CCB_7FD5, // raster reference
    0x0CCB_7FD6, // base tile
];
const TERRAIN_A: std::ops::RangeInclusive<u32> = 0x0D02_D586..=0x0D02_D58B;
const TERRAIN_B: std::ops::RangeInclusive<u32> = 0x0D7A_F042..=0x0D7A_F049;

fn allowed(hash: u32) -> bool {
    PARCEL_FIELDS.contains(&hash)
        || hash == 0x0A02_09A4
        || TERRAIN_A.contains(&hash)
        || TERRAIN_B.contains(&hash)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let input = args.next().ok_or("usage: trim_ground_lot <in.package> <out.package>")?;
    let output = args.next().ok_or("usage: trim_ground_lot <in.package> <out.package>")?;

    let pkg = Package::open(Path::new(&input))?;
    let mut entries = Vec::new();
    for entry in pkg.entries() {
        let bytes = pkg.read(entry)?;
        if entry.id.type_id == 0x00B1_B104 {
            let mut file = PropertyFile::parse(&bytes)?;
            let before = file.values.len();
            file.values.retain(|prop| allowed(prop.hash));
            file.claimed_count = file.values.len() as u32;
            println!(
                "trim {:08X}:{:08X}:{:08X}: {} -> {} fields (dropped: {})",
                entry.id.type_id,
                entry.id.group,
                entry.id.instance,
                before,
                file.values.len(),
                before - file.values.len(),
            );
            entries.push(dbpf::OverlayEntry::new(entry.id, file.encode_canonical()?));
        } else {
            entries.push(OverlayEntry::new(entry.id, bytes));
        }
    }
    let payload = write_uncompressed_overlay(&entries)?;
    std::fs::write(&output, payload)?;
    println!("wrote {output}");
    Ok(())
}
