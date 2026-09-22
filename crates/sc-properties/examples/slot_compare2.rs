//! 同包内比较两类型的 instance 集合。仅开发用。
use std::collections::HashSet;
fn slots(path: &str, ty: u32, group: u32) -> HashSet<u32> {
    let p = dbpf::Package::open(path).unwrap();
    p.entries().iter().filter(|e| e.id.type_id == ty && e.id.group == group).map(|e| e.id.instance).collect()
}
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let s1 = slots(&a[0], u32::from_str_radix(a[1].trim_start_matches("0x"), 16).unwrap(), u32::from_str_radix(a[2].trim_start_matches("0x"), 16).unwrap());
    let s2 = slots(&a[0], u32::from_str_radix(a[3].trim_start_matches("0x"), 16).unwrap(), u32::from_str_radix(a[4].trim_start_matches("0x"), 16).unwrap());
    println!("type1={} type2={} common={} only-in-type1={} only-in-type2={}", s1.len(), s2.len(), s1.intersection(&s2).count(), s1.difference(&s2).count(), s2.difference(&s1).count());
}
