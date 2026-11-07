//! Dump raw per-property layout (type byte, flags, count, item_size, value
//! bytes in hex) for chosen hashes of a lot resource — parser-level ground
//! truth for comparing vanilla columns against pipeline-written ones.
//! Usage: cargo run -q -p fluffy-open-scp --example lot_column_bytes --
//!       <package> <type:group:instance> <hash1,hash2,...>
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropType, PropertyFile};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let parts: Vec<u32> = args[2]
        .split(':')
        .map(|p| u32::from_str_radix(p.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let hashes: Vec<u32> = args[3]
        .split(',')
        .map(|h| u32::from_str_radix(h.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let id = ResourceId { type_id: parts[0], group: parts[1], instance: parts[2] };
    let entry = package.entry(id).expect("entry not found");
    let bytes = package.read(entry).expect("read entry");
    let file = PropertyFile::parse(&bytes).expect("parse");
    println!("# file: {} entries, claimed_count={}", file.values.len(), file.claimed_count);
    // stored order (file order, not sorted) — first 40 hashes
    let order: Vec<String> = file.values.iter().take(40).map(|p| format!("{:08X}", p.hash)).collect();
    println!("# stored order (first 40): {}", order.join(" "));
    for property in &file.values {
        if !hashes.contains(&property.hash) { continue; }
        let (count, item_size) = match &property.kind {
            Kind::Array(vs) => (vs.len(), property.encoding.array_item_size),
            Kind::Scalar(_) => (1, None),
            Kind::Empty => (0, None),
        };
        println!(
            "{:08X} type={:?} entry_flags={:#06x} count={} item_size={:?}",
            property.hash, property.prop_type, property.encoding.flags, count, item_size
        );
        if let Kind::Array(vs) = &property.kind {
            if let Some(v) = vs.first() {
                println!("  first={v:?}");
            }
        }
    }
}
