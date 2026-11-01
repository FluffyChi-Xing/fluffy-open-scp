//! Build a tiny identity donor package from Dalian's three properties and
//! locale table.  Only used by the manual park fixtures.

use dbpf::{OverlayEntry, ResourceId, write_uncompressed_overlay};
use std::{env, fs, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = env::args().nth(1).ok_or("usage: make_identity_donor <dir> <out>")?;
    let out = env::args().nth(2).ok_or("usage: make_identity_donor <dir> <out>")?;
    let files = [
        (0x00B1_B104, 0x0987_8A01, 0xF922_1FCF, "00B1B104-09878A01-F9221FCF.bin"),
        (0x00B1_B104, 0x40E0_C000, 0xF922_1FCF, "00B1B104-40E0C000-F9221FCF.bin"),
        (0x00B1_B104, 0x40E1_C000, 0xF922_1FCF, "00B1B104-40E1C000-F9221FCF.bin"),
        (0x0A98_EAF0, 0x02FA_BF01, 0x284C_6727, "0A98EAF0-02FABF01-284C6727.bin"),
    ];
    let entries: Vec<_> = files
        .iter()
        .map(|(type_id, group, instance, name)| {
            OverlayEntry::new(
                ResourceId {
                    type_id: *type_id,
                    group: *group,
                    instance: *instance,
                },
                fs::read(Path::new(&dir).join(name)).unwrap(),
            )
        })
        .collect();
    fs::write(out, write_uncompressed_overlay(&entries)?)?;
    Ok(())
}
