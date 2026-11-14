//! 一次性对拍实验 R2：NightHouse 原包逐字节保留，仅 lot 叶子内 4 个 LOD
//! 模型 key（BE 字节）替换为我们的 merged-single 模型。
//! 用法：cargo run -p rw4 --release --example nh_lodswap_build -- <nh_pkg> <patched_lot> <out_pkg>
use dbpf::{OverlayEntry, Package, write_uncompressed_overlay_to_path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (nh_path, lot_path, out_path) = (&args[0], &args[1], &args[2]);
    let patched_lot = std::fs::read(lot_path)?;

    let pkg = Package::open(nh_path)?;
    let mut entries = Vec::new();
    for e in pkg.entries() {
        let payload = if e.id.type_id == 0x00B1_B104
            && e.id.group == 0x40E1_C000
            && e.id.instance == 0xA420_E06D
        {
            patched_lot.clone()
        } else {
            pkg.read(e)?
        };
        entries.push(OverlayEntry::new(e.id, payload));
    }
    write_uncompressed_overlay_to_path(out_path, &entries)?;
    println!("written {} entries -> {}", entries.len(), out_path);
    Ok(())
}
