//! 一次性探针：在给定 package 中按 instance 定位资源（组 40E0C400/40E1C400
//! 的 PathEntry 普查）。用法：
//! `cargo run -p dbpf --example find_tgi -- <package> <instance-hex>...`

fn main() {
    let mut args = std::env::args().skip(1);
    let package = args.next().expect("package path");
    let needles: Vec<u32> = args
        .map(|value| u32::from_str_radix(value.trim_start_matches("0x"), 16).expect("hex instance"))
        .collect();
    let pkg = dbpf::Package::open(&package).expect("open package");
    for entry in pkg.entries() {
        if needles.contains(&entry.id.instance) {
            println!(
                "{:08X}:{:08X}:{:08X}  size={}",
                entry.id.type_id, entry.id.group, entry.id.instance, entry.decompressed_size
            );
        }
    }
}
