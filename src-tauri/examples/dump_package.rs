//! Dump every entry TGI + SHA-256 of a DBPF package (audit tooling).
//! Usage: cargo run --release -p fluffy-open-scp --example dump_package -- <a.package> <b.package>

use dbpf::Package;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn dump(path: &std::path::Path) -> BTreeMap<String, (u64, String)> {
    let package = Package::open(path).expect("open package");
    let mut out = BTreeMap::new();
    for entry in package.entries() {
        let bytes = package.read(entry).expect("read entry");
        let hash: String = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        out.insert(
            format!(
                "{:08X}:{:08X}:{:08X}",
                entry.id.type_id, entry.id.group, entry.id.instance
            ),
            (bytes.len() as u64, hash),
        );
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let a = dump(std::path::Path::new(&args[1]));
    let only_a = args.get(2).is_none();
    let b = args
        .get(2)
        .map(|path| dump(std::path::Path::new(path)))
        .unwrap_or_default();
    println!(
        "{}: {} entries | {}: {} entries",
        args[1],
        a.len(),
        args.get(2).map(String::as_str).unwrap_or("-"),
        b.len()
    );
    for (key, (size, hash)) in &a {
        match b.get(key) {
            None => println!("  only-A {key} {size} B"),
            Some((bsize, bhash)) => {
                if only_a {
                    println!("  {key} {size} B {hash}");
                } else if bhash == hash {
                    println!("  same    {key} {size} B");
                } else {
                    println!(
                        "  CHANGED {key} A {size} B {} vs B {bsize} B {}",
                        &hash[..12],
                        &bhash[..12]
                    );
                }
            }
        }
    }
    for (key, (size, _)) in &b {
        if !a.contains_key(key) {
            println!("  only-B {key} {size} B");
        }
    }
}
