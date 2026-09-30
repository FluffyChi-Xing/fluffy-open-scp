//! 冒烟工具：全包全类型搜索某 instance 的资源（列 TGI+大小+RW4 材质槽）。
//! 用法：cargo run -p sc-exporter --release --example find_instance_all -- <instance_hex> <pkg> [...]
use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let instance = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    for path in &args[1..] {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        for e in package.entries() {
            if e.id.instance != instance { continue; }
            let data = package.read(e).unwrap_or_default();
            println!("{path}: T {:08X} G {:08X} ({}B)", e.id.type_id, e.id.group, data.len());
            if e.id.type_id == 0x2F4E_681B {
                if let Ok(file) = rw4::Rw4File::parse(&data) {
                    for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
                        match file.decode_material(&data, section.number) {
                            Ok(rw4::MaterialSection::Decoded(m)) => {
                                for r in &m.texture_refs {
                                    println!("  slot {:#04x} -> {:#010x}", r.slot, r.texture_instance);
                                }
                            }
                            Ok(rw4::MaterialSection::Raw(_)) => println!("  (raw)"),
                            Err(err) => println!("  err {err}"),
                        }
                    }
                }
            }
        }
    }
}
