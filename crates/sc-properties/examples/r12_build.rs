//! R12 最小变量实验：R1 实证菜单包（原件全部保留，TGI 不动）+ 模型载荷
//! 换 R9 已验证塔楼 + NH 纹理条目。判别：崩溃源在模型（菜单语境）还是
//! R11 菜单资源。
use dbpf::{OverlayEntry, Package, write_uncompressed_overlay_to_path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let r1 = Package::open(
        "tmp/asset-build-fixtures/commercial-model/.build/fc209ccc34e1512ceb4b1b2ec22d4343f7dfd582b982c11eee65c4e455e4a7c9/packages/commercial-menu-park.package",
    )?;
    let nh = Package::open("D:/ea-games/SimCity/SimCityData/SC5_NightHouse_Test_0_5_0.package")?;
    let tower = std::fs::read("tmp/nh_r9_tower.rw4")?;
    println!("tower: {} bytes", tower.len());
    let mut entries: Vec<OverlayEntry> = Vec::new();
    for e in r1.entries() {
        let payload = if e.id.type_id == 0x2F4E_681B && e.id.group == 0 && r1.read(e)?.len() > 4096
        {
            println!(
                "swapping model {:08X} ({})",
                e.id.instance,
                r1.read(e)?.len()
            );
            tower.clone()
        } else {
            r1.read(e)?
        };
        entries.push(OverlayEntry::new(e.id, payload));
    }
    for e in nh.entries() {
        if e.id.type_id == 0x2F4E_681C {
            println!(
                "adding NH texture {:08X} ({}B)",
                e.id.instance,
                nh.read(e)?.len()
            );
            entries.push(OverlayEntry::new(e.id, nh.read(e)?));
        }
    }
    let out = "tmp/nh_r12_minimal.package";
    write_uncompressed_overlay_to_path(std::path::Path::new(out), &entries)?;
    println!("R12: {} entries -> {}", entries.len(), out);
    Ok(())
}
