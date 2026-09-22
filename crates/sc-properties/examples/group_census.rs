//! 按 group 统计 property 数（找区域组）。仅开发用。
use std::collections::HashMap;
fn main() {
    let path = std::env::args().nth(1).expect("package");
    let p = dbpf::Package::open(&path).unwrap();
    let mut m: HashMap<u32, usize> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x00B1_B104 { *m.entry(e.id.group).or_default() += 1; }
    }
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (g, n) in v.iter().take(20) { println!("{g:08X}: {n}"); }
}
