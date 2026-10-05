//! 冒烟工具：定位 instance 在哪个包哪个组（含尺寸）。
//! 用法：cargo run -p sc-exporter --release --example find_instance_loc -- <instance_hex> <pkg> [...]
use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let instance = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    for path in &args[1..] {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        for e in package.entries().iter().filter(|e| e.id.instance == instance) {
            println!(
                "{path}: T {:08X} G {:08X} ({}B)",
                e.id.type_id,
                e.id.group,
                e.decompressed_size
            );
        }
    }
}
