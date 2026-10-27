//! 探针：跨包按 instance 搜资源（任意 type/group）。仅开发用。
fn main() {
    let inst = u32::from_str_radix(std::env::args().nth(1).unwrap().trim_start_matches("0x"), 16).unwrap();
    for path in std::env::args().skip(2) {
        let p = dbpf::Package::open(&path).unwrap();
        for e in p.entries() {
            if e.id.instance == inst {
                println!("{} -> TGI {:08X}:{:08X}:{:08X} 压缩={} 解压={}", path, e.id.type_id, e.id.group, e.id.instance, e.compressed_size, p.read(e).map(|d| d.len()).unwrap_or(0));
            }
        }
    }
}
