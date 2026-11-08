//! Full bin table (07E0AD23/24) of a bundle + search for specific resources.
//! Usage: cargo run -q -p fluffy-open-scp --example bundle_bins -- <package> <bundle-instance> [needle-hex...]
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let parts: Vec<u32> = args[2]
        .split(':')
        .map(|p| u32::from_str_radix(p.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let needles: Vec<u32> = args
        .iter()
        .skip(3)
        .map(|n| u32::from_str_radix(n.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let instance = u32::from_str_radix(
        args[2].rsplit(':').next().unwrap().trim_start_matches("0x"),
        16,
    )
    .unwrap();
    let id = ResourceId {
        type_id: 0x00B1_B104,
        group: 0x40E0_C700,
        instance,
    };
    let entry = package.entry(id).expect("bundle not found");
    let file = PropertyFile::parse(&package.read(entry).unwrap()).expect("parse");
    let ids = match file.get(0x07E0_AD23).map(|p| &p.kind) {
        Some(Kind::Array(values)) => values
            .iter()
            .filter_map(|v| match v {
                Value::Key(k) => Some(k.instance),
                _ => None,
            })
            .collect::<Vec<u32>>(),
        _ => Vec::new(),
    };
    let caps = match file.get(0x07E0_AD24).map(|p| &p.kind) {
        Some(Kind::Array(values)) => values
            .iter()
            .filter_map(|v| match v {
                Value::UInt32(x) => Some(*x),
                Value::Int32(x) => Some(*x as u32),
                _ => None,
            })
            .collect::<Vec<u32>>(),
        _ => Vec::new(),
    };
    println!("bins: {} (caps: {})", ids.len(), caps.len());
    for (index, instance) in ids.iter().enumerate() {
        let cap = caps.get(index).copied().unwrap_or(0);
        let needle = if needles.contains(instance) {
            " <<车辆"
        } else {
            ""
        };
        println!("  [{index:02}] {instance:08X} cap {cap}{needle}");
    }
}
