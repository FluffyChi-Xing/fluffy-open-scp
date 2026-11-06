//! Field-level diff of one property resource across two packages.
//! Usage: cargo run --release -p fluffy-open-scp --example property_diff -- <a.package> <b.package> <type:group:instance>

use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

fn load(path: &str, id: ResourceId) -> PropertyFile {
    let package = Package::open(std::path::Path::new(path)).expect("open");
    let entry = package.entry(id).expect("entry");
    PropertyFile::parse(&package.read(entry).expect("read")).expect("parse")
}

fn render_value(value: &Value) -> String {
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

fn render(file: &PropertyFile) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for property in &file.values {
        let rendered = match &property.kind {
            Kind::Empty => "empty".to_string(),
            Kind::Scalar(value) => render_value(value),
            Kind::Array(values) => values.iter().map(render_value).collect::<Vec<_>>().join(", "),
        };
        map.insert(format!("{:08X}", property.hash), rendered);
    }
    map
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let parts: Vec<u32> = args[3]
        .split(':')
        .map(|part| u32::from_str_radix(part.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let id = ResourceId {
        type_id: parts[0],
        group: parts[1],
        instance: parts[2],
    };
    let (ra, rb) = (render(&load(&args[1], id)), render(&load(&args[2], id)));
    println!("== {id}");
    let keys: std::collections::BTreeSet<_> = ra.keys().chain(rb.keys()).collect();
    for key in keys {
        let va = ra.get(key).map(String::as_str).unwrap_or("<absent>");
        let vb = rb.get(key).map(String::as_str).unwrap_or("<absent>");
        if va != vb {
            println!("  {key}  A [{va}]");
            println!("       > B [{vb}]");
        }
    }
    println!("== done");
}
