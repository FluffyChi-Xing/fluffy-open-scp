//! 探针：dump 一个 property 资源的原始（解压后）字节，可选与另一资源逐字节
//! 对比，输出差异偏移。用于对拍拷贝属性与原版属性的编码差异。仅开发用。
//!
//! 用法：cargo run -p probes --release --example prop_bytes_diff --
//!   <a.package> <a.instance_hex> <b.package> <b.instance_hex> [group_hex]
use dbpf::Package;

fn fetch(path: &str, inst: u32, group: u32) -> Vec<u8> {
    let p = Package::open(path).unwrap();
    for e in p.entries() {
        if e.id.type_id == 0x00B1_B104 && e.id.instance == inst && e.id.group == group {
            println!(
                "{path} {:08X}: stored={} raw={} compressed={}",
                inst,
                e.stored_len(),
                p.read(e).unwrap().len(),
                e.compressed
            );
            return p.read(e).unwrap();
        }
    }
    panic!("未找到 {:08X} 于 {path}", inst);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let group = args
        .get(4)
        .map(|g| u32::from_str_radix(g.trim_start_matches("0x"), 16).unwrap())
        .unwrap_or(0x0987_8A01);
    let hex = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let a = fetch(&args[0], hex(&args[1]), group);
    let b = fetch(&args[2], hex(&args[3]), group);
    let n = a.len().max(b.len());
    let mut diffs = 0;
    for i in 0..n {
        let (x, y) = (a.get(i).copied(), b.get(i).copied());
        if x != y {
            diffs += 1;
            println!(
                "off {:#06x}: {:02X} vs {:02X}  (a={:08X} b={:08X})",
                i,
                x.unwrap_or(0),
                y.unwrap_or(0),
                hex(&args[1]),
                hex(&args[3])
            );
        }
    }
    println!("总长 {} vs {}，差异字节数 {}", a.len(), b.len(), diffs);
}
