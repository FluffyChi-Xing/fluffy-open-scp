//! 冒烟工具：按 TGI 抽资源前 N 字节做 16 进制 + ASCII dump（类型取证）。
//!
//! 用法：cargo run -p sc-exporter --release --example tgi_peek -- <package> <type>:<group>:<instance> [len]

use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, rest) = args
        .split_first()
        .expect("usage: tgi_peek <package> <type>:<group>:<instance> [len]");
    let tgi = rest.first().expect("need tgi");
    let len: usize = rest.get(1).map(|v| v.parse().unwrap_or(256)).unwrap_or(256);
    let package = Package::open(path).expect("open package");
    if tgi == "list" {
        use std::collections::BTreeMap;
        let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
        for e in package.entries() {
            *counts.entry(e.id.type_id).or_default() += 1;
        }
        for (type_id, count) in counts {
            println!("{type_id:08x}  x{count}");
        }
        return;
    }
    let parts: Vec<Option<u32>> = tgi
        .split(':')
        .map(|part| {
            if part == "*" {
                None
            } else {
                Some(u32::from_str_radix(part.trim_start_matches("0x"), 16).expect("hex"))
            }
        })
        .collect();
    assert_eq!(parts.len(), 3, "tgi must be type:group:instance");
    let entry = package
        .entries()
        .iter()
        .find(|e| {
            parts[0].map_or(true, |v| e.id.type_id == v)
                && parts[1].map_or(true, |v| e.id.group == v)
                && parts[2].map_or(true, |v| e.id.instance == v)
        })
        .cloned()
        .unwrap_or_else(|| panic!("resource not found"));
    println!(
        "match type={:08x} group={:08x} instance={:08x}",
        entry.id.type_id, entry.id.group, entry.id.instance
    );
    let raw = package.read(&entry).expect("read resource");
    println!("tgi={tgi} stored={} decompressed={}", entry.stored_len(), raw.len());
    let data: Vec<u8> = if raw.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = flate2::read::GzDecoder::new(&raw[..]);
        let mut out = Vec::new();
        std::io::Read::read_to_end(&mut decoder, &mut out).expect("gzip");
        println!("gzip payload: {} bytes", out.len());
        out
    } else {
        raw
    };
    let mut run = String::new();
    let mut strings = Vec::new();
    for &b in &data {
        if (0x20..0x7f).contains(&b) {
            run.push(char::from_u32(b as u32).unwrap());
        } else {
            if run.len() >= 6 {
                strings.push(run.clone());
            }
            run.clear();
        }
    }
    for (index, text) in strings.iter().take(40).enumerate() {
        println!("str[{index:02}] {text}");
    }
    if data.len() > len * 4 {
        let tail = &data[data.len() - len..];
        for (offset, chunk) in tail.chunks(16).enumerate() {
            let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
            println!("tail {:08x}  {:<47}", data.len() - len + offset * 16, hex.join(" "));
        }
    }
    let head = &data[..data.len().min(len)];
    for (offset, chunk) in head.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    char::from_u32(b as u32).unwrap()
                } else {
                    '.'
                }
            })
            .collect();
        println!("{:08x}  {:<47}  {}", offset * 16, hex.join(" "), ascii);
    }
}
