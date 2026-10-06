//! 冒烟工具：列出某 group 的全部 B1B104 记录。
//! 用法：cargo run -p sc-exporter --release --example list_group -- <group_hex> <pkg> [...]
use dbpf::Package;
use sc_properties::{PropertyFile, ParseLimits, Kind, Value};
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let group = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    for path in &args[1..] {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        let mut count = 0usize;
        let mut by_parent: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for e in package.entries().iter().filter(|e| {
            e.id.type_id == 0x00B1_B104 && e.id.group == group
        }) {
            count += 1;
            let Ok(data) = package.read(e) else { continue };
            let Ok(f) = PropertyFile::parse_with_limits(&data, ParseLimits::default()) else {
                continue;
            };
            let parent = f.values.iter()
                .find(|p| p.hash == 0x00B2_CCCB)
                .and_then(|p| match &p.kind {
                    Kind::Scalar(Value::Key(k)) => Some(format!("{:08X}", k.instance)),
                    _ => None,
                })
                .unwrap_or_else(|| "-".into());
            by_parent.entry(parent).or_default().push(format!("{:08X}", e.id.instance));
        }
        println!("{path}: {count} 条");
        for (parent, ids) in &by_parent {
            println!("  parent {parent}: {}", ids.join(" "));
        }
    }
}
