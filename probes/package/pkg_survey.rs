//! 探针：包类型直方图 + 按 instance 前缀统计。仅开发用。
fn main() {
    for path in std::env::args().skip(1) {
        let p = dbpf::Package::open(&path).unwrap();
        let mut hist: std::collections::HashMap<u32, (usize, u64)> = std::collections::HashMap::new();
        for e in p.entries() {
            let h = hist.entry(e.id.type_id).or_default();
            h.0 += 1;
            h.1 += e.compressed_size as u64;
        }
        let mut v: Vec<_> = hist.iter().collect();
        v.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
        println!("== {} ({} 条目)", path, p.entries().len());
        for (t, (n, sz)) in v.iter().take(25) {
            println!("  {:08X} ×{} {}B", t, n, sz);
        }
    }
}
