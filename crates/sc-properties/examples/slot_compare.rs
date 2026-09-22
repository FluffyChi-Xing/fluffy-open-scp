//! 对比两包 341-tile 组的 instance 集合。仅开发用。
use std::collections::HashSet;
fn slots(path: &str, group: u32) -> HashSet<u32> {
    let p = dbpf::Package::open(path).unwrap();
    p.entries().iter()
        .filter(|e| e.id.type_id == 0x03E4_21F0 && e.id.group == group)
        .map(|e| e.id.instance).collect()
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a = slots(&args[0], u32::from_str_radix(args[1].trim_start_matches("0x"), 16).unwrap());
    let b = slots(&args[2], u32::from_str_radix(args[3].trim_start_matches("0x"), 16).unwrap());
    println!("A={} B={} common={} onlyA={} onlyB={}", a.len(), b.len(), a.intersection(&b).count(), a.difference(&b).count(), b.difference(&a).count());
    let only_b: Vec<u32> = b.difference(&a).copied().collect();
    println!("onlyB sample: {:?}", &only_b[..only_b.len().min(10)]);
}
