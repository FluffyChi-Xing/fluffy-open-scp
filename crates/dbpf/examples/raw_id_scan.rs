//! 探针：在包内**全部资源**的原始字节里搜索指定 u32（LE/BE 两种字节序）。
//! 用于判定某个 id 是否只被 lot 属性文件引用，还是另有「目录/表」类资源承载。
//! 仅开发用。
//!
//! 用法：cargo run -p dbpf --release --example raw_id_scan -- <package> 0xID[,0xID...]
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: raw_id_scan <package> 0xID[,0xID...]");
    let ids: Vec<u32> = args
        .next()
        .expect("need ids")
        .split(',')
        .map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let package = dbpf::Package::open(&path).expect("open package");

    let mut hits_by_type: BTreeMap<u32, usize> = BTreeMap::new();
    let mut total = 0usize;
    for entry in package.entries() {
        let Ok(data) = package.read(entry) else { continue };
        for id in &ids {
            let le = id.to_le_bytes();
            let be = id.to_be_bytes();
            let mut count = 0usize;
            if data.len() >= 4 {
                for i in 0..=data.len() - 4 {
                    let w = &data[i..i + 4];
                    if w == le || w == be {
                        count += 1;
                    }
                }
            }
            if count > 0 {
                total += 1;
                *hits_by_type.entry(entry.id.type_id).or_default() += 1;
                println!(
                    "id 0x{:08X} × {count:<4} in {} type=0x{:08X} group=0x{:08X} instance=0x{:08X} size={}",
                    id,
                    path.split(['/', '\\']).next_back().unwrap_or(&path),
                    entry.id.type_id,
                    entry.id.group,
                    entry.id.instance,
                    data.len()
                );
            }
        }
    }
    println!("\n--- 命中资源 {total} 个；按 type 分布：");
    for (t, n) in &hits_by_type {
        println!("    type=0x{t:08X} × {n}");
    }
}
