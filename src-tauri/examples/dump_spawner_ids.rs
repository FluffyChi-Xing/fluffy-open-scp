//! Dump spawner anchor id lists (0E1BAC61) for comparing agent types.
//! Usage: ... -- <package> <type:group:instance>
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let parts: Vec<u32> = args[2].split(':').map(|p| u32::from_str_radix(p.trim_start_matches("0x"),16).unwrap()).collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let id = ResourceId { type_id: parts[0], group: parts[1], instance: parts[2] };
    let bytes = package.read(package.entry(id).expect("e")).expect("r");
    let file = PropertyFile::parse(&bytes).expect("parse");
    let ids: Vec<u32> = file.get(0x0E1B_AC61).and_then(|p| match &p.kind {
        Kind::Array(vs) => Some(vs.iter().filter_map(|v| match v { Value::Key(k) => Some(k.instance), _ => None }).collect()),
        _ => None }).unwrap_or_default();
    println!("# {} spawner ids", ids.len());
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    let mut grouped: Vec<(u32, usize)> = Vec::new();
    for v in sorted { match grouped.last_mut() { Some((last, n)) if *last == v => *n += 1, _ => grouped.push((v, 1)) } }
    for (v, n) in grouped { println!("  {v:08X} x{n}"); }
}
