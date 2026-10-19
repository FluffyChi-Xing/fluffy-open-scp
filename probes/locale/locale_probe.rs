//! 探针：检查 zh-tw locale 包里重复表条目与具体字符串内容。仅开发用。
use dbpf::Package;
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let p = args.next().expect("usage: locale_probe <Data.package>");
    let package = Package::open(&p).unwrap();
    let mut counts: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    let mut idx = 0usize;
    for e in package.entries() {
        idx += 1;
        let i = idx;
        if e.id.type_id == 0x0A98_EAF0 {
            counts.entry(e.id.instance).or_default().push(i);
        }
    }
    for table in [0x50AA0BEAu32, 0x6C969DEE, 0x7B45C298] {
        println!("{table:08X} copies: {:?}", counts.get(&table).map(|v| v.len()));
    }
    for e in package.entries() {
        if e.id.type_id == 0x0A98_EAF0 && e.id.instance == 0x6C969DEE {
            let d = package.read(e).unwrap();
            let s = String::from_utf8_lossy(&d);
            for key in ["\"0x0d364987\"", "\"0x0d2e3fd0\""] {
                match s.find(key) {
                    Some(pos) => println!("6C969DEE {}: {}", key, &s[pos..pos + 70.min(s.len() - pos)]),
                    None => println!("6C969DEE {}: 无", key),
                }
            }
        }
        if e.id.type_id == 0x0A98_EAF0 && e.id.instance == 0x50AA0BEA {
            let d = package.read(e).unwrap();
            let s = String::from_utf8_lossy(&d);
            let key = "\"0x0ef02395\"";
            match s.find(key) {
                Some(pos) => println!(
                    "copy@{}B: {}",
                    e.compressed_size,
                    &s[pos..pos + 90.min(s.len() - pos)]
                ),
                None => println!("copy@{}B: 无 0x0ef02395", e.compressed_size),
            }
        }
    }
}
