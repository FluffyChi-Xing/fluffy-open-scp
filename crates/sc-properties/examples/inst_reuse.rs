//! 检查同类型 instance 跨 group 复用情况。仅开发用。
use std::collections::HashMap;
fn main() {
    let path = std::env::args().nth(1).expect("package");
    let ty = u32::from_str_radix(std::env::args().nth(2).expect("type").trim_start_matches("0x"), 16).unwrap();
    let package = dbpf::Package::open(&path).expect("open");
    let mut by_inst: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut total = 0;
    for e in package.entries() {
        if e.id.type_id == ty {
            total += 1;
            by_inst.entry(e.id.instance).or_default().push(e.id.group);
        }
    }
    let multi = by_inst.values().filter(|v| v.len() > 1).count();
    println!("type 0x{ty:08X}: {total} entries, {} unique instances, {multi} reused by >1 group", by_inst.len());
    // sample a reused instance
    for (inst, groups) in by_inst.iter().take(4000) {
        if groups.len() > 1 {
            println!("inst 0x{inst:08X} in {} groups: {:?}", groups.len(), &groups[..groups.len().min(4)]);
            break;
        }
    }
}
