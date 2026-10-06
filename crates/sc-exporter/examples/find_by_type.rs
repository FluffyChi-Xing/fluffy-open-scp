//! 冒烟工具：按 type_id 列出全部资源（计数 + 按 group 分布 + 抽样 instance）。
//! 用法：cargo run -p sc-exporter --release --example find_by_type -- <type_hex> <pkg> [...]
use dbpf::Package;
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let type_id = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    let mut total = 0usize;
    let mut groups: BTreeMap<u32, usize> = BTreeMap::new();
    let mut samples: Vec<String> = Vec::new();
    for path in &args[1..] {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        for e in package.entries().iter().filter(|e| e.id.type_id == type_id) {
            total += 1;
            *groups.entry(e.id.group).or_default() += 1;
            if samples.len() < 8 {
                samples.push(format!("{:08X}", e.id.instance));
            }
        }
    }
    println!("type {type_id:08X}: 共 {total} 条");
    for (g, n) in &groups {
        println!("  G {g:08X}: {n}");
    }
    println!("样本: {}", samples.join(" "));
}
