//! Unit 属性全量 dump——解析 0xB1B104 属性表，输出语义分类、全部属性
//! （hash/kind/值），并与原版包同 TGI 逐属性比对。
//! 用法：cargo run -p sc-exporter --release --example prop_dump -- <mod_pkg> [vanilla_pkg]
use dbpf::Package;
use sc_properties::{property_semantic, Kind, PropertyFile, Value};

const PROP_TYPE: u32 = 0x00B1_B104;

fn values_line(kind: &Kind) -> String {
    match kind {
        Kind::Scalar(v) => format!("{v}"),
        Kind::Array(vs) => {
            let items: Vec<String> = vs.iter().map(|v| v.to_string()).collect();
            if items.len() <= 8 {
                format!("[{}]", items.join(", "))
            } else {
                format!("[{} 项] 前几项: [{}]", items.len(), items[..4].join(", "))
            }
        }
        Kind::Empty => "(empty)".into(),
    }
}

fn dump_file(pkg: &Package, group: u32, instance: u32, vanilla: Option<&Package>) {
    let Some(e) = pkg
        .entries()
        .iter()
        .find(|e| e.id.type_id == PROP_TYPE && e.id.group == group && e.id.instance == instance)
        .cloned()
    else {
        println!("  TGI 不在本包");
        return;
    };
    let data = pkg.read(&e).expect("read");
    let pf = match PropertyFile::parse(&data) {
        Ok(pf) => pf,
        Err(err) => {
            println!("  解析失败 {err}");
            return;
        }
    };
    let sem = property_semantic(&pf, group);
    println!(
        "  语义: {:?}({})  属性数 {}",
        sem,
        sem.label_zh(),
        pf.values.len()
    );
    let mut vanilla_map: std::collections::BTreeMap<u32, String> = Default::default();
    let mut vanilla_count = 0usize;
    if let Some(vp) = vanilla {
        if let Some(ve) = vp
            .entries()
            .iter()
            .find(|e| e.id.type_id == PROP_TYPE && e.id.group == group && e.id.instance == instance)
            .cloned()
        {
            if let Ok(vdata) = vp.read(&ve) {
                if let Ok(vpf) = PropertyFile::parse(&vdata) {
                    vanilla_count = vpf.values.len();
                    for p in &vpf.values {
                        vanilla_map.insert(p.hash, values_line(&p.kind));
                    }
                }
            }
        }
    }
    let mut added = 0usize;
    let mut changed = 0usize;
    for p in &pf.values {
        let cur = values_line(&p.kind);
        let tag = match vanilla_map.get(&p.hash) {
            None => {
                added += 1;
                if vanilla_count > 0 {
                    "[新增]"
                } else {
                    ""
                }
            }
            Some(old) if *old != cur => {
                changed += 1;
                "[改动]"
            }
            _ => "",
        };
        let mark: String = if tag.is_empty() { String::new() } else { format!("{tag} ") };
        println!("    {}{:#010x} = {}", mark, p.hash, cur);
    }
    if vanilla_count > 0 {
        println!(
            "    与原版同 TGI 对比: 新增 {added} / 改动 {changed} / 原版属性数 {vanilla_count}"
        );
    } else if vanilla.is_some() {
        println!("    原版无同 TGI（新增条目）");
    }
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (mod_path, van_path) = match a.as_slice() {
        [m] => (m.clone(), None),
        [m, v] => (m.clone(), Some(v.clone())),
        _ => panic!("用法: prop_dump <mod_pkg> [vanilla_pkg]"),
    };
    let mp = Package::open(&mod_path).expect("open mod");
    let vp = van_path.map(|v| Package::open(&v).expect("open vanilla"));
    let mut entries: Vec<(u32, u32)> = mp
        .entries()
        .iter()
        .filter(|e| e.id.type_id == 0x00B1_B104)
        .map(|e| (e.id.group, e.id.instance))
        .collect();
    entries.sort_unstable();
    for (idx, (group, instance)) in entries.iter().enumerate() {
        println!("#{} G {group:08X} I {instance:08X}", idx + 1);
        dump_file(&mp, *group, *instance, vp.as_ref());
        println!();
    }
}
