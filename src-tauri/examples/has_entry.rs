//! Check whether a TGI exists in a package.
//! Usage: cargo run -q -p fluffy-open-scp --example has_entry -- <package> <type:group:instance>
use dbpf::{Package, ResourceId};

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
    match package.entry(id) {
        Some(entry) => println!("FOUND {} ({} B)", id, package.read(entry).unwrap().len()),
        None => println!("ABSENT {id}"),
    }
}
