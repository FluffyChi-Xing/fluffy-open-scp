//! 按 instance 找资源（全类型）。仅开发用。
fn main() {
    let mut args = std::env::args().skip(1);
    let inst = u32::from_str_radix(args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    for path in args {
        let Ok(package) = dbpf::Package::open(&path) else { continue };
        for entry in package.entries() {
            if entry.id.instance == inst {
                println!("{}: type=0x{:08X} group=0x{:08X} stored={}", path, entry.id.type_id, entry.id.group, entry.compressed_size);
            }
        }
    }
}
