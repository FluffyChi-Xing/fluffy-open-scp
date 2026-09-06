//! 检查同 InstanceId 的 property 资源分布：变体 lot 是否有兄弟资源携带 placement。仅开发用。
use std::collections::HashMap;

fn main() {
    let path = std::env::args().nth(1).expect("usage: lot_sibling_probe <package> [instances...]");
    let package = dbpf::Package::open(&path).expect("open package");
    let targets: Vec<u32> = std::env::args().skip(2)
        .filter_map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .collect();
    let mut by_instance: HashMap<u32, Vec<(u32, u32)>> = HashMap::new(); // instance -> [(group, size)]
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 { continue; }
        by_instance.entry(entry.id.instance).or_default().push((entry.id.group, entry.compressed_size));
    }
    for t in &targets {
        let sibs = by_instance.get(t);
        println!("0x{t:08X}: {} same-instance property resources", sibs.map(|v| v.len()).unwrap_or(0));
        for (g, s) in sibs.into_iter().flatten() {
            println!("    group=0x{g:08X} stored={s}B");
        }
    }
    // 全包统计：多少 property instance 有 >1 条资源
    let multi = by_instance.values().filter(|v| v.len() > 1).count();
    println!("--- property instances={} with_multiple_resources={multi}", by_instance.len());
}
