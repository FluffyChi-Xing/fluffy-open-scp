//! Read/extract/build the single scrui resource; never modifies an input package.
use dbpf::{OverlayEntry, Package, ResourceId};
use std::{error::Error, fs, io::Write, path::Path};

const ID: ResourceId = ResourceId {
    type_id: 0x6777_1F5C,
    group: 0x4046_4200,
    instance: 0x7D14_BE70,
};

fn create(path: &str, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("scan") && args.len() == 2 {
        for path in fs::read_dir(&args[1])? {
            let path = path?.path();
            if path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("package"))
            {
                let package = Package::open(&path)?;
                if package.entries().iter().any(|e| e.id == ID) {
                    println!("{}", path.display());
                }
            }
        }
        return Ok(());
    }
    if args.len() != 2 && args.len() != 4 {
        return Err("usage: ui_observer_package <app.package> <baseline.js> [<suffix.js> <output.package>] | scan <directory>".into());
    }
    let package = Package::open(&args[0])?;
    let entries: Vec<_> = package.entries().iter().filter(|e| e.id == ID).collect();
    if entries.len() != 1 {
        return Err("scrui resource must occur exactly once".into());
    }
    let baseline = package.read(entries[0])?;
    if args.len() == 2 {
        return create(&args[1], &baseline);
    }
    if baseline != fs::read(&args[1])? {
        return Err("baseline mismatch".into());
    }
    if Path::new(&args[3]).exists() {
        return Err("output already exists".into());
    }
    let mut patched = baseline;
    let suffix = fs::read(&args[2])?;
    if !suffix.is_empty() {
        patched.extend_from_slice(b"\n;\n");
        patched.extend_from_slice(&suffix);
    }
    let bytes = dbpf::write_uncompressed_overlay(&[OverlayEntry::new(ID, patched.clone())])?;
    create(&args[3], &bytes)?;
    let output = Package::open(&args[3])?;
    if output.entries().len() != 1
        || output.entries()[0].id != ID
        || output.read(&output.entries()[0])? != patched
    {
        return Err("overlay roundtrip mismatch".into());
    }
    Ok(())
}
