//! Dump every property of a unit (parsed, sorted by hash) for A/B comparison.
//! Usage: ... -- <package> <type:group:instance>
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let parts: Vec<u32> = args[2]
        .split(':')
        .map(|p| u32::from_str_radix(p.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let bytes = package
        .read(
            package
                .entry(ResourceId {
                    type_id: parts[0],
                    group: parts[1],
                    instance: parts[2],
                })
                .expect("e"),
        )
        .expect("r");
    let file = PropertyFile::parse(&bytes).expect("parse");
    let mut props: Vec<_> = file.values.iter().collect();
    props.sort_by_key(|p| p.hash);
    for p in props {
        let brief = match &p.kind {
            Kind::Array(vs) => {
                let items: Vec<String> = vs
                    .iter()
                    .take(6)
                    .map(|v| {
                        let s = format!("{v:?}");
                        s.chars().take(48).collect()
                    })
                    .collect();
                format!("array[{}] {}", vs.len(), items.join(" | "))
            }
            other => {
                let s = format!("{other:?}");
                s.chars().take(90).collect()
            }
        };
        println!("{:08X} {}", p.hash, brief);
    }
}
