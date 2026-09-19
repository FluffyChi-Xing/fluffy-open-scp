//! 探针：按完整 TGI 打印单个资源的属性（PropertyFile 调试视图）。仅开发用。
//!
//! 用法：cargo run -p sc-properties --release --example prop_dump -- <package> <type> <group> <instance>
//! （type/group/instance 接受 0x 前缀十六进制；group 传 0 即可匹配 00000000）

use sc_properties::PropertyFile;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, rest) = args.split_first().expect("usage: prop_dump <package> <type> <group> <instance>");
    let nums: Vec<u32> = rest
        .iter()
        .map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).expect("bad hex"))
        .collect();
    let (ty, group, inst) = (nums[0], nums[1], nums[2]);
    let package = dbpf::Package::open(path).expect("open package");
    for entry in package.entries() {
        if entry.id.type_id == ty && entry.id.group == group && entry.id.instance == inst {
            println!(
                "found: type=0x{:08X} group=0x{:08X} instance=0x{:08X} stored={}",
                entry.id.type_id, entry.id.group, entry.id.instance, entry.compressed_size
            );
            let data = package.read(&entry).expect("read entry");
            match PropertyFile::parse(&data) {
                Ok(file) => print!("{file}"),
                Err(e) => println!("not a property file: {e} ({} bytes)", data.len()),
            }
            return;
        }
    }
    eprintln!("entry 0x{ty:08X}/0x{group:08X}/0x{inst:08X} not found");
    std::process::exit(1);
}
