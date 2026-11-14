//! Count entries of one type across a package (audit tooling).
//! Usage: cargo run --release -p fluffy-open-scp --example scan_type -- <package> <hex-type> [hex-group]
use dbpf::Package;
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let want: u32 = u32::from_str_radix(args[2].trim_start_matches("0x"), 16).unwrap();
    let group: Option<u32> = args
        .get(3)
        .map(|g| u32::from_str_radix(g.trim_start_matches("0x"), 16).unwrap());
    let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
    for entry in package.entries() {
        if entry.id.type_id != want {
            continue;
        }
        if let Some(g) = group {
            if entry.id.group != g {
                continue;
            }
        }
        *counts.entry(entry.id.group).or_default() += 1;
    }
    for (group, count) in counts {
        println!("group {group:08X}: {count}");
    }
    for entry in package.entries() {
        if entry.id.type_id == want && group.map(|g| entry.id.group == g).unwrap_or(true) {
            println!("  instance {:08X}", entry.id.instance);
        }
    }
}
