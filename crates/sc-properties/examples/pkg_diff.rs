//! 整包逐条目对比（TGI 集合 + 解压后内容字节比对）。仅开发用。
//!
//! 用法：cargo run -p sc-properties --release --example pkg_diff -- <vanilla> <modded>
use std::collections::{BTreeMap, BTreeSet};

fn load(path: &str) -> BTreeMap<(u32, u32, u32), Vec<u8>> {
    let package = dbpf::Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
    let mut map = BTreeMap::new();
    for entry in package.entries() {
        let content = package.read(entry).expect("read entry");
        map.insert(
            (entry.id.type_id, entry.id.group, entry.id.instance),
            content,
        );
    }
    map
}

fn main() {
    let mut args = std::env::args().skip(1);
    let van = args.next().expect("usage: pkg_diff <vanilla> <modded>");
    let modded = args.next().expect("usage: pkg_diff <vanilla> <modded>");
    let a = load(&van);
    let b = load(&modded);
    println!("vanilla entries: {}, modded entries: {}", a.len(), b.len());
    let ka: BTreeSet<_> = a.keys().collect();
    let kb: BTreeSet<_> = b.keys().collect();
    for k in ka.difference(&kb) {
        println!("ONLY-IN-VANILLA {:08X}:{:08X}:{:08X}", k.0, k.1, k.2);
    }
    for k in kb.difference(&ka) {
        println!("ONLY-IN-MOD    {:08X}:{:08X}:{:08X}", k.0, k.1, k.2);
    }
    for k in ka.intersection(&kb) {
        let (va, vb) = (&a[*k], &b[*k]);
        if va != vb {
            let diff_bytes = va.iter().zip(vb).filter(|(x, y)| x != y).count();
            println!(
                "DIFF {:08X}:{:08X}:{:08X}  dec {} vs {} bytes, differing bytes {}",
                k.0,
                k.1,
                k.2,
                va.len(),
                vb.len(),
                diff_bytes
            );
        }
    }
}
