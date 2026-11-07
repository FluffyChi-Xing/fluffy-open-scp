//! Dump a property resource by TGI from a package.
//! Usage: cargo run -q -p fluffy-open-scp --example dump_by_tgi -- <package> <type:group:instance>
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};

fn render(v: &Value) -> String {
    match v {
        Value::Int32(x) => format!("i32:{x}"),
        Value::UInt32(x) => format!("u32:{x}"),
        Value::Float(x) => format!("f32:{x}"),
        Value::Bool(x) => format!("bool:{x}"),
        Value::Key(k) => format!("key({:08X},{:08X},{:08X})", k.type_id, k.group, k.instance),
        Value::Text(t) => format!("text({:08X},{})", t.table_id, t.instance_id),
        _ => format!("{v:?}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let parts: Vec<u32> = args[2]
        .split(':')
        .map(|p| u32::from_str_radix(p.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let id = ResourceId { type_id: parts[0], group: parts[1], instance: parts[2] };
    let entry = package.entry(id).expect("entry not found");
    let file = PropertyFile::parse(&package.read(entry).unwrap()).expect("parse");
    let mut hashes: Vec<u32> = file.values.iter().map(|p| p.hash).collect();
    hashes.sort_unstable();
    for hash in hashes {
        let property = file.get(hash).unwrap();
        let rendered = match &property.kind {
            Kind::Empty => "empty".into(),
            Kind::Scalar(v) => render(v),
            Kind::Array(vs) => {
                if vs.len() <= 20 {
                    vs.iter().map(render).collect::<Vec<_>>().join(", ")
                } else {
                    format!("[{}] {} … {}", vs.len(), render(&vs[0]), render(&vs[vs.len() - 1]))
                }
            }
        };
        println!("{hash:08X} {} {rendered}", if property.encoding.flags & 0x8000 != 0 { "V" } else { " " });
    }
}
