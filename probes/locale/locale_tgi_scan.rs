//! 探针：列出 locale 包内 0x0A98EAF0 资源的完整 TGI 分布。仅开发用。
use dbpf::Package;
use sc_properties::locale::LOCALE_RESOURCE_TYPE;
use std::collections::BTreeMap;

fn main() {
    let path = std::env::args().nth(1).expect("locale Data.package");
    let p = Package::open(&path).unwrap();
    let mut by_group: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for e in p.entries() {
        if e.id.type_id == LOCALE_RESOURCE_TYPE {
            by_group.entry(e.id.group).or_default().push(e.id.instance);
        }
    }
    println!("{} locale entries", p.entries().iter().filter(|e| e.id.type_id == LOCALE_RESOURCE_TYPE).count());
    for (g, mut insts) in by_group {
        insts.sort();
        println!("group {g:08X}: {} tables, first few: {:?}",
            insts.len(),
            insts.iter().take(8).map(|i| format!("{i:08X}")).collect::<Vec<_>>()
        );
    }
    // 三个目标表的完整 TGI
    for t in [0x4B54417Au32, 0x50AA0BEA, 0x6C969DEE] {
        for e in p.entries() {
            if e.id.type_id == LOCALE_RESOURCE_TYPE && e.id.instance == t {
                println!("table {t:08X}: TGI={:08X}:{:08X}:{:08X} stored={} raw={} compressed={}",
                    e.id.type_id, e.id.group, e.id.instance, e.stored_len(),
                    p.read(e).unwrap().len(), e.compressed);
            }
        }
    }
}
