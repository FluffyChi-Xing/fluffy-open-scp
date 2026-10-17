//! 手动取证：把 Swarm 效果集合资源原样落盘（供字符串/结构分析）。
//! cargo run -p sc-properties --release --example effect_collection_dump -- <pkg> <group_hex> <instance_hex> <out>
use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let package = Package::open(&args[0]).expect("open package");
    let group = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).expect("group");
    let instance = u32::from_str_radix(args[2].trim_start_matches("0x"), 16).expect("instance");
    for entry in package.entries() {
        if entry.id.type_id == 0xEA51_18B0
            && entry.id.group == group
            && entry.id.instance == instance
        {
            let data = package.read(entry).expect("read");
            std::fs::write(&args[3], &data).expect("write");
            println!("written {} bytes -> {}", data.len(), args[3]);
            return;
        }
    }
    panic!("collection not found");
}
