//! 导出 locale 包内全部 0x0A98EAF0 表为 JSON（table instance → {key: text}）。仅开发用。
//!
//! 用法：cargo run -p probes --release --example locale_tables_dump -- <locale.package> <out.json>

use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: locale_tables_dump <locale.package> <out.json>");
    let out = args.next().expect("out.json");
    let p = dbpf::Package::open(&path).expect("open locale package");
    let mut tables: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for e in p.entries() {
        if e.id.type_id != 0x0A98_EAF0 {
            continue;
        }
        let Ok(data) = p.read(e) else { continue };
        let text = String::from_utf8_lossy(&data);
        let text = text.trim_start_matches('\u{feff}');
        let Ok(map) = serde_json::from_str::<BTreeMap<String, String>>(text) else {
            println!("skip table {:08X}: not flat string map", e.id.instance);
            continue;
        };
        tables.insert(format!("{:08X}", e.id.instance), map);
    }
    let json = serde_json::to_string(&tables).expect("serialize");
    std::fs::write(&out, json).expect("write out");
    println!("{} tables -> {out}", tables.len());
}
