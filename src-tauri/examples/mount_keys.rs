use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let instance = u32::from_str_radix(args[2].trim_start_matches("0x"), 16).unwrap();
    let bytes = package
        .read(
            package
                .entry(ResourceId {
                    type_id: 0x00B1_B104,
                    group: 0x48E0_C000,
                    instance,
                })
                .expect("e"),
        )
        .expect("r");
    let file = PropertyFile::parse(&bytes).expect("parse");
    for hash in [0x0A02_09A2u32, 0x0A02_09A3, 0x0A02_09A4, 0x0A02_09A5] {
        if let Some(Kind::Array(vs)) = file.get(hash).map(|p| &p.kind) {
            println!(
                "{hash:08X}: {}",
                vs.iter()
                    .map(|v| match v {
                        Value::Key(k) => format!(
                            "key(inst={:08X} type={:08X} grp={:08X})",
                            k.instance, k.type_id, k.group
                        ),
                        other => format!("{other:?}"),
                    })
                    .collect::<Vec<_>>()
                    .join(" | ")
            );
        }
    }
}
