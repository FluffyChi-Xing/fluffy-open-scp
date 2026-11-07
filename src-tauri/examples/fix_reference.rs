//! Replace a mutated template entry in a reference package with the clean
//! vanilla-form bytes from a source package.
//! Usage: ... -- <clean-source.package> <hex-instance> <reference-in> <reference-out>
use dbpf::{OverlayEntry, Package, ResourceId};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let source = Package::open(std::path::Path::new(&args[1])).expect("open source");
    let want = u32::from_str_radix(args[2].trim_start_matches("0x"), 16).unwrap();
    let reference = Package::open(std::path::Path::new(&args[3])).expect("open reference");
    let out_path = &args[4];

    let clean = source
        .entries()
        .iter()
        .find(|e| e.id.instance == want)
        .expect("clean entry not found in source");
    let clean_bytes = source.read(clean).expect("read clean");

    let mut entries = Vec::new();
    let mut replaced = false;
    for entry in reference.entries() {
        let bytes = reference.read(entry).expect("read entry");
        if entry.id.instance == want && entry.id.type_id == clean.id.type_id {
            replaced = true;
            println!(
                "replace {:08X}:{:08X}:{:08X} {}B -> {}B",
                entry.id.type_id,
                entry.id.group,
                entry.id.instance,
                bytes.len(),
                clean_bytes.len()
            );
            entries.push(OverlayEntry::new(entry.id, clean_bytes.clone()));
        } else {
            entries.push(OverlayEntry::new(entry.id, bytes));
        }
    }
    assert!(replaced, "target instance not found in reference");
    let payload = dbpf::write_uncompressed_overlay(&entries).expect("write overlay");
    std::fs::write(out_path, payload).expect("write out");
    println!("wrote {out_path}");
}
