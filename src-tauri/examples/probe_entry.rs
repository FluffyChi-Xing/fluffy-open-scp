use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let parts: Vec<u32> = args[2]
        .split(':')
        .map(|p| u32::from_str_radix(p.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let id = ResourceId {
        type_id: parts[0],
        group: parts[1],
        instance: parts[2],
    };
    let bytes = package.read(package.entry(id).expect("e")).expect("r");
    match PropertyFile::parse(&bytes) {
        Err(e) => println!("parse Err: {e}"),
        Ok(file) => {
            println!("props={}", file.values.len());
            for p in &file.values {
                if let Kind::Array(vs) = &p.kind {
                    println!(
                        "{:08X} array[{}] stride={:?}",
                        p.hash,
                        vs.len(),
                        p.encoding.array_item_size
                    );
                }
            }
        }
    }
}
