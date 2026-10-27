//! 探针：在包内**解压后**的资源负载里搜二进制字节模式（payload_grep 的二进制版）。
//! 命中打印 TGI + 条目内偏移 + 前后文 hex。用于在压缩条目内定位属性 key /
//! 结构体字段等裸字节。仅开发用。
//!
//! 用法：
//!   cargo run -p probes --release --example payload_bin_grep -- <hex>[,<hex>...] <package...>
//!   [--limit=N]   每个 needle 每包最多打印多少条命中（默认 8）

use std::collections::BTreeMap;
use std::io::Read;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let limit: usize = args
        .iter()
        .position(|a| a.starts_with("--limit="))
        .and_then(|i| {
            let v = args[i]["--limit=".len()..].parse().ok();
            args.remove(i);
            v
        })
        .unwrap_or(8);
    let needle_arg = args.first().expect("usage: payload_bin_grep <hex[,hex...]> <package...>").clone();
    let raw = args.iter().position(|a| a == "--raw").map(|i| args.remove(i)).is_some();
    let gzip = args.iter().position(|a| a == "--gzip").map(|i| args.remove(i)).is_some();
    let needles: Vec<(String, Vec<u8>)> = needle_arg
        .split(',')
        .map(|h| {
            let h = h.trim_start_matches("0x");
            (
                format!("0x{h}"),
                (0..h.len() / 2)
                    .map(|i| u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap())
                    .collect::<Vec<u8>>(),
            )
        })
        .collect();
    for path in &args[1..] {
        if raw {
            let data = std::fs::read(path).expect("read file");
            let p = dbpf::Package::open(path).expect("open package");
            for (label, pat) in &needles {
                let mut from = 0usize;
                while let Some(off) = find(&data, from, pat) {
                    let owner = p
                        .entries()
                        .iter()
                        .find(|e| {
                            off as u64 >= e.offset
                                && (off as u64) < e.offset + e.stored_len()
                        })
                        .map(|e| {
                            format!(
                                "{:08X}/{:08X} compressed={}",
                                e.id.group, e.id.instance, e.compressed
                            )
                        })
                        .unwrap_or_else(|| "GAP(index/header/padding)".into());
                    let s = off.saturating_sub(16);
                    let e2 = (off + pat.len() + 16).min(data.len());
                    println!(
                        "{} needle {} @ file:{:x} in [{}] ctx: {}",
                        file_name(path),
                        label,
                        off,
                        owner,
                        data[s..e2].iter().map(|b| format!("{b:02x}")).collect::<String>()
                    );
                    from = off + 1;
                }
            }
            continue;
        }
        let p = dbpf::Package::open(path).expect("open package");
        let mut read_failures = 0usize;
        let mut per_needle: BTreeMap<String, usize> = BTreeMap::new();
        for e in p.entries() {
            let data = match p.read(&e) {
                Ok(d) => d,
                Err(_) => {
                    read_failures += 1;
                    continue;
                }
            };
            let scan_targets: Vec<Vec<u8>> = if gzip {
                let mut v = vec![data.clone()];
                let mut from = 0usize;
                while let Some(g) = find(&data, from, &[0x1f, 0x8b]) {
                    let mut dec = Vec::new();
                    if flate2::read::GzDecoder::new(&data[g..])
                        .read_to_end(&mut dec)
                        .is_ok()
                        && dec.len() > 64
                    {
                        v.push(dec);
                    }
                    from = g + 1;
                }
                v
            } else {
                vec![data.clone()]
            };
            for blob in &scan_targets {
                for (label, pat) in &needles {
                    let mut from = 0usize;
                    while let Some(off) = find(blob, from, pat) {
                        let n = per_needle.entry(label.clone()).and_modify(|c| *c += 1).or_insert(1);
                        if *n <= limit {
                            let s = off.saturating_sub(16);
                            let e2 = (off + pat.len() + 16).min(blob.len());
                            println!(
                                "{} needle {} @ {}:{}#{:x} ctx[{}..{}]: {}",
                                file_name(path),
                                label,
                                e.id.type_id,
                                e.id.instance,
                                off,
                                s,
                                e2,
                                blob[s..e2].iter().map(|b| format!("{b:02x}")).collect::<String>()
                            );
                        }
                        from = off + 1;
                    }
                }
            }
        }
        for (label, n) in &per_needle {
            println!("{} needle {} total {}", file_name(path), label, n);
        }
        println!(
            "{} entries={} read_failures={}",
            file_name(path),
            p.entries().len(),
            read_failures
        );
    }
}

fn find(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

fn file_name(p: &str) -> &str {
    std::path::Path::new(p).file_name().and_then(|s| s.to_str()).unwrap_or(p)
}
