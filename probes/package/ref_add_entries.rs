//! 一次性工具:把来源包中的指定条目并入目标 DBPF 包(现有条目保留)。
//! 用法:cargo run -p probes --example ref_add_entries -- <target.package> <source.package> <t1:g1:i1> [t2:g2:i2 ...]
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use std::{collections::BTreeMap, path::Path};

fn parse_tgi(s: &str) -> ResourceId {
    let parts: Vec<u32> = s
        .split(':')
        .map(|x| u32::from_str_radix(x.trim().trim_start_matches("0x"), 16).expect("hex tgi"))
        .collect();
    ResourceId { type_id: parts[0], group: parts[1], instance: parts[2] }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let target_path = args.next().expect("target package");
    let source_path = args.next().expect("source package");
    let wanted: Vec<ResourceId> = args.map(|x| parse_tgi(&x)).collect();
    let source = Package::open(Path::new(&source_path)).expect("open source");
    let target = Package::open(Path::new(&target_path)).expect("open target");

    let mut map: BTreeMap<(u32, u32, u32), OverlayEntry> = BTreeMap::new();
    for entry in target.entries() {
        let bytes = target.read(entry).expect("read");
        map.insert(
            (entry.id.type_id, entry.id.group, entry.id.instance),
            OverlayEntry::new(entry.id, bytes),
        );
    }
    for id in &wanted {
        let entry = source
            .entry(*id)
            .unwrap_or_else(|| panic!("source missing {id:08X?}"));
        let bytes = source.read(entry).expect("read");
        println!(
            "+ {:08X}:{:08X}:{:08X} {}B",
            id.type_id, id.group, id.instance,
            bytes.len()
        );
        map.insert(
            (id.type_id, id.group, id.instance),
            OverlayEntry::new(*id, bytes),
        );
    }
    let entries: Vec<OverlayEntry> = map.into_values().collect();
    drop(target); // ERROR_USER_MAPPED_FILE:写回前必须释放内存映射
    std::fs::write(
        &target_path,
        write_uncompressed_overlay(&entries).unwrap(),
    )
    .unwrap();
    println!("wrote {} entries", entries.len());
}
