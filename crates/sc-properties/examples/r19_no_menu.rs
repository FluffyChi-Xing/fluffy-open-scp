//! R19 isolation package: remove Menu2/Unit/icon/locale from R18 while
//! retaining the Lot leaf, model and material resources.

use dbpf::{OverlayEntry, Package, write_uncompressed_overlay_to_path};
use std::path::Path;

fn keep(id: dbpf::ResourceId) -> bool {
    !(id.type_id == 0x00B1_B104 && (id.group == 0x0987_8A01 || id.group == 0x40E0_C000))
        && !(id.type_id == 0x2F7D_0004 && id.group == 0x40E0_2400)
        && !(id.type_id == 0x3F86_62EA && id.group == 0)
        && !(id.type_id == 0x0A98_EAF0 && id.group == 0x02FA_BF01)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "tmp/000_r18_known_model.package".into());
    let output = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "tmp/000_r19_no_menu.package".into());
    let source = Package::open(&input)?;
    let mut entries = Vec::new();
    for entry in source.entries() {
        if keep(entry.id) {
            entries.push(OverlayEntry::new(entry.id, source.read(entry)?));
        }
    }
    let path = Path::new(&output);
    write_uncompressed_overlay_to_path(path, &entries)?;
    println!(
        "R19: {} entries -> {} (Menu2/Unit/icon/locale removed)",
        entries.len(),
        path.display()
    );
    Ok(())
}
