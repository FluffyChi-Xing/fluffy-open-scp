//! 冒烟工具：从 s3db 注册表按 instance 查资产名。
//!
//! 用法：cargo run -p sc-registry --release --example name_lookup -- \
//!   <database_main.s3db> 0xC2D6D136 [0x...]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (registry_path, instances) = args.split_first().expect("usage: name_lookup <s3db> 0xID...");
    let registry = sc_registry::Registry::open(registry_path).expect("open registry");
    for arg in instances {
        let Ok(id) = u32::from_str_radix(arg.trim_start_matches("0x"), 16) else {
            continue;
        };
        let name = registry
            .instances()
            .get(&id)
            .map(|record| record.name.as_str())
            .unwrap_or("(unknown)");
        println!("0x{id:08X} -> {name}");
    }
}
