//! 探针：合并多个包的全部条目为单个 overlay 包。TGI 冲突时后输入的包覆盖
//! 先输入的（与游戏扁平 TGI 表的覆盖语义一致）。仅开发用。
//!
//! 用法：cargo run -p probes --release --example pkg_merge --
//!   <out.package> <in1.package> [in2.package ...]
use dbpf::{OverlayEntry, Package};
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().expect("out path");
    let mut map: BTreeMap<(u32, u32, u32), (String, OverlayEntry)> = BTreeMap::new();
    for path in args {
        let p = Package::open(&path).expect("open input");
        let mut n = 0;
        for e in p.entries() {
            let data = p.read(e).expect("read");
            let key = (e.id.type_id, e.id.group, e.id.instance);
            if map
                .insert(key, (path.clone(), OverlayEntry::new(e.id, data)))
                .is_some()
            {
                println!("覆盖 TGI {:08X}:{:08X}:{:08X} <- {path}", key.0, key.1, key.2);
            }
            n += 1;
        }
        println!("{path}: {n} 条目");
    }
    let entries: Vec<OverlayEntry> = map.into_values().map(|(_, e)| e).collect();
    let bytes = dbpf::write_uncompressed_overlay(&entries).expect("write");
    std::fs::write(&out, &bytes).expect("write file");
    println!("写出 {out}: {} 条目 {}B", entries.len(), bytes.len());
}
