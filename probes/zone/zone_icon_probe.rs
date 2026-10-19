//! 探针：字节等价搜索 —— 在多个包里找与目标 PNG 完全相同的资源，定位其来源 TGI。仅开发用。
fn main() {
    let mut args = std::env::args().skip(1);
    let target_path = std::path::PathBuf::from(args.next().expect("usage: zone_icon_probe <target.png> <package...>"));
    let target = std::fs::read(&target_path).expect("read target");
    println!("target {} B", target.len());
    for path in args {
        let Ok(package) = dbpf::Package::open(&path) else { continue };
        for entry in package.entries() {
            if entry.id.type_id != 0x2F7D_0004 {
                continue;
            }
            let Ok(data) = package.read(entry) else { continue };
            if data == target {
                println!(
                    "HIT {}: type=0x{:08X} group=0x{:08X} instance=0x{:08X}",
                    path, entry.id.type_id, entry.id.group, entry.id.instance
                );
            }
        }
    }
}
