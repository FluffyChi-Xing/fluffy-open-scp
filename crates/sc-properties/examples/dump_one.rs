//! 探针：按 TGI 转储单条资源。仅开发用。
fn main() {
    let mut a = std::env::args().skip(1);
    let path = a.next().unwrap();
    let t = u32::from_str_radix(a.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let g = u32::from_str_radix(a.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let i = u32::from_str_radix(a.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let p = dbpf::Package::open(&path).unwrap();
    for e in p.entries() {
        if e.id.type_id == t && e.id.group == g && e.id.instance == i {
            let d = p.read(e).unwrap();
            let out = a.next().unwrap_or_else(|| format!("tmp/dump_{i:08X}.bin"));
            std::fs::write(&out, &d).unwrap();
            println!("{:08X}:{:08X}:{:08X} {} B -> {}", t, g, i, d.len(), out);
            return;
        }
    }
    println!("未找到");
}
