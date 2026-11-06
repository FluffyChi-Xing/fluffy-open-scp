//! Dump every field of one property resource.
//! Usage: cargo run --release -p fluffy-open-scp --example dump_property -- <package> <type:group:instance>
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};

fn render(value: &Value) -> String {
    match value {
        Value::Int32(v) => format!("i32:{v}"),
        Value::UInt32(v) => format!("u32:{v}"),
        Value::Float(v) => format!("f32:{v}"),
        Value::Bool(v) => format!("bool:{v}"),
        Value::Key(key) => format!("key({:08X},{:08X},{:08X})", key.type_id, key.group, key.instance),
        Value::Text(text) => format!("text({:08X},{})", text.table_id, text.instance_id),
        _ => format!("{value:?}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let parts: Vec<u32> = args[2]
        .split(':')
        .map(|part| u32::from_str_radix(part.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let id = ResourceId {
        type_id: parts[0],
        group: parts[1],
        instance: parts[2],
    };
    let entry = package.entry(id).expect("entry");
    let file = PropertyFile::parse(&package.read(entry).expect("read")).expect("parse");
    let mut hashes: Vec<u32> = file.values.iter().map(|p| p.hash).collect();
    hashes.sort_unstable();
    for hash in hashes {
        let property = file.get(hash).unwrap();
        let rendered = match &property.kind {
            Kind::Empty => "empty".to_string(),
            Kind::Scalar(value) => render(value),
            Kind::Array(values) => {
                if values.len() <= 24 {
                    values.iter().map(render).collect::<Vec<_>>().join(", ")
                } else {
                    format!(
                        "[{} values] {} … {}",
                        values.len(),
                        render(&values[0]),
                        render(&values[values.len() - 1])
                    )
                }
            }
        };
        println!(
            "{hash:08X} flags={:04X} size={:2} {}",
            property.encoding.flags,
            property.encoding.array_item_size.unwrap_or(0),
            rendered
        );
    }
}
