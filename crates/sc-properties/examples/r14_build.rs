//! R14 = R12 减 NH 纹理：R1 实证包 + 仅模型载荷换塔楼。
use dbpf::{OverlayEntry, Package, write_uncompressed_overlay_to_path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let r1 = Package::open(
        "tmp/asset-build-fixtures/commercial-model/.build/fc209ccc34e1512ceb4b1b2ec22d4343f7dfd582b982c11eee65c4e455e4a7c9/packages/commercial-menu-park.package",
    )?;
    let tower = std::fs::read("tmp/nh_r9_tower.rw4")?;
    let mut entries: Vec<OverlayEntry> = Vec::new();
    for e in r1.entries() {
        let payload = if e.id.type_id == 0x2F4E_681B && e.id.group == 0 && r1.read(e)?.len() > 4096
        {
            tower.clone()
        } else {
            r1.read(e)?
        };
        entries.push(OverlayEntry::new(e.id, payload));
    }
    let out = "tmp/nh_r14_notex.package";
    write_uncompressed_overlay_to_path(std::path::Path::new(out), &entries)?;
    println!("R14: {} entries -> {}", entries.len(), out);
    Ok(())
}
