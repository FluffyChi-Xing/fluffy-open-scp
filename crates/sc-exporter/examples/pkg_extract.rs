//! 冒烟工具：按 TGI 抽取条目原文。
//! 用法：cargo run -p sc-exporter --release --example pkg_extract -- <pkg> <type_hex> <group_hex> <instance_hex> <out_file>
use dbpf::Package;

fn parse_hex(s: &str) -> u32 {
    let t = s.trim_start_matches("0x").trim_start_matches("0X");
    u32::from_str_radix(t, 16).expect("hex")
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (path, t, g, i, out) = match a.as_slice() {
        [p, t, g, i, o] => (p.clone(), parse_hex(t), parse_hex(g), parse_hex(i), o.clone()),
        _ => panic!("用法: pkg_extract <pkg> <type> <group> <instance> <out>"),
    };
    let pkg = Package::open(&path).expect("open");
    let e = pkg
        .entries()
        .iter()
        .find(|e| e.id.type_id == t && e.id.group == g && e.id.instance == i)
        .cloned()
        .expect("条目不存在");
    let data = pkg.read(&e).expect("read");
    std::fs::write(&out, &data).expect("write");
    println!("{} -> {} ({}B)", path, out, data.len());
}
