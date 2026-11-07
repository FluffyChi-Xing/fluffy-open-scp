//! Scan vanilla lots for bin-prop column patterns (0C12EF21) and multi-path
//! encodings (0CC8FE7A len>=2) to establish ground-truth encodings.
//! Usage: cargo run -q -p fluffy-open-scp --example scan_lot_patterns -- <package> [max-hits]
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};

fn lens(file: &PropertyFile, hash: u32) -> Option<usize> {
    file.get(hash).map(|p| match &p.kind {
        Kind::Array(vs) => vs.len(),
        Kind::Scalar(_) => 1,
        Kind::Empty => 0,
    })
}

fn keys(file: &PropertyFile, hash: u32) -> Vec<u32> {
    match file.get(hash).map(|p| &p.kind) {
        Some(Kind::Array(vs)) => vs
            .iter()
            .filter_map(|v| match v {
                Value::Key(k) => Some(k.instance),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn ints(file: &PropertyFile, hash: u32) -> Vec<i32> {
    match file.get(hash).map(|p| &p.kind) {
        Some(Kind::Array(vs)) => vs
            .iter()
            .filter_map(|v| match v {
                Value::Int32(x) => Some(*x),
                Value::UInt32(x) => Some(*x as i32),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let max_hits: usize = args
        .get(2)
        .and_then(|v| v.parse().ok())
        .unwrap_or(12);
    let package = Package::open(std::path::Path::new(&args[1])).expect("open package");
    let mut bin_hits = 0;
    let mut path_hits = 0;
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 || entry.id.group != 0x40E1_C000 {
            continue;
        }
        let bytes = package.read(entry).expect("read entry");
        let Ok(file) = PropertyFile::parse(&bytes) else { continue };
        let has_bin1 = file.get(0x0C12_EF21).is_some();
        let big_bin = lens(&file, 0x0C12_EF20).unwrap_or(0) >= 2
            || lens(&file, 0x0C12_EF21).unwrap_or(0) >= 2;
        let entries = keys(&file, 0x0CC8_FE7A);
        let multi_path = entries.len() >= 2;
        if !has_bin1 && !multi_path && !big_bin {
            continue;
        }
        let id = format!(
            "{:08X}:{:08X}:{:08X}",
            entry.id.type_id, entry.id.group, entry.id.instance
        );
        if (has_bin1 || big_bin) && bin_hits < max_hits {
            bin_hits += 1;
            println!(
                "BIN {id} 21keys={:?} len20={:?} len21={:?} len30={:?} len31={:?} len40={:?} len41={:?} bools(50/5C/60/6C)={:?}/{:?}/{:?}/{:?}",
                keys(&file, 0x0C12_EF21),
                lens(&file, 0x0C12_EF20),
                lens(&file, 0x0C12_EF21),
                lens(&file, 0x0C12_EF30),
                lens(&file, 0x0C12_EF31),
                lens(&file, 0x0C12_EF40),
                lens(&file, 0x0C12_EF41),
                file.get(0x0C12_EF50).map(|p| &p.kind),
                file.get(0x0C12_EF5C).map(|p| &p.kind),
                file.get(0x0C12_EF60).map(|p| &p.kind),
                file.get(0x0C12_EF6C).map(|p| &p.kind),
            );
        }
        if multi_path && path_hits < max_hits {
            path_hits += 1;
            println!(
                "PATH {id} entries={:?} len0D={:?} len0DAA680D={:?} len0CB00ED8={:?} 6832={:?} 6841={:?} road(0D9AA2BB)={:?} scope={:?}",
                entries,
                lens(&file, 0x0D9A_4E54),
                lens(&file, 0x0CAA_680D),
                lens(&file, 0x0CB0_0ED8),
                ints(&file, 0x0CAA_6832),
                ints(&file, 0x0CAA_6841),
                ints(&file, 0x0D9A_A2BB),
                ints(&file, 0x0D9A_4E54),
            );
        }
        if bin_hits >= max_hits && path_hits >= max_hits {
            break;
        }
    }
    println!("# bin hits shown: {bin_hits}, multi-path hits shown: {path_hits}");
    let _ = ResourceId { type_id: 0, group: 0, instance: 0 };
}
