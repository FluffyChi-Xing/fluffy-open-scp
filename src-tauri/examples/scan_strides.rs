//! Calibrate the legal declared-stride table: collect (PropType -> strides)
//! across property-file groups the pipeline writes, over the whole base game.
//! Usage: cargo run -q -p fluffy-open-scp --example scan_strides -- <package>
use dbpf::Package;
use sc_properties::{Kind, PropType, PropertyFile};
use std::collections::BTreeMap;

const GROUPS: [u32; 5] = [0x40E0_C000, 0x40E1_C000, 0x40E0_C400, 0x40E0_C700, 0x0987_8A01];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    // (group, type) -> stride -> occurrences
    let mut table: BTreeMap<(u32, &'static str), BTreeMap<i32, usize>> = BTreeMap::new();
    let mut scalar_types: BTreeMap<(u32, &'static str), usize> = BTreeMap::new();
    let mut files = 0usize;
    let mut parse_failures = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 || !GROUPS.contains(&entry.id.group) {
            continue;
        }
        let Ok(bytes) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&bytes) else {
            parse_failures += 1;
            continue;
        };
        files += 1;
        let group = entry.id.group;
        for property in &file.values {
            let type_name: &'static str = Box::leak(format!("{:?}", property.prop_type).into_boxed_str());
            match &property.kind {
                Kind::Array(vs) => {
                    let stride_value = property.encoding.array_item_size.unwrap_or(0);
                    let slot = table.entry((group, type_name)).or_default();
                    *slot.entry(stride_value).or_insert(0) += vs.len().max(1);
                }
                Kind::Scalar(_) => {
                    *scalar_types.entry((group, type_name)).or_default() += 1;
                }
                Kind::Empty => {
                    *scalar_types.entry((group, "EMPTY")).or_default() += 1;
                }
            }
        }
    }
    println!("# files={files} parse_failures={parse_failures}");
    for ((group, type_name), strides) in &table {
        let rendered: Vec<String> = strides
            .iter()
            .map(|(stride, count)| format!("stride={stride} x{count}"))
            .collect();
        println!("{group:08X} {type_name}: {}", rendered.join(", "));
    }
    for ((group, type_name), count) in &scalar_types {
        println!("{group:08X} {type_name}: scalar x{count}");
    }
}
