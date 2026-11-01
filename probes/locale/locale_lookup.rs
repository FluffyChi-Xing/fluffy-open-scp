//! 探针：在某个 locale Data.package 的全部 0x0A98EAF0 字符串表里按 text id 反查文本。
//! 用途：验证 transport entry 上的活动文案列（如 0x0E4F02C2）指向哪些本地化字符串。
//!
//! 用法：cargo run -p probes --example locale_lookup -- <Data.package> <id-hex,...>
use dbpf::Package;
use sc_properties::locale;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: locale_lookup <Data.package> <ids>");
    let ids: Vec<u32> = args
        .next()
        .expect("ids")
        .split(',')
        .map(|s| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).expect("hex"))
        .collect();
    let pkg = Package::open(&path).expect("open locale package");
    let mut tables: Vec<(u32, std::collections::HashMap<u32, String>)> = Vec::new();
    for e in pkg.entries() {
        if e.id.type_id != locale::LOCALE_RESOURCE_TYPE {
            continue;
        }
        let Ok(data) = pkg.read(e) else { continue };
        if let Ok(map) = locale::parse_string_table(&data) {
            tables.push((e.id.instance, map));
        }
    }
    println!("locale tables: {}", tables.len());
    for id in &ids {
        let mut any = false;
        for (tid, map) in &tables {
            if let Some(text) = map.get(id) {
                any = true;
                println!("0x{id:08X} => [table 0x{tid:08X}] {text}");
            }
        }
        if !any {
            println!("0x{id:08X} => (not found)");
        }
    }
}
