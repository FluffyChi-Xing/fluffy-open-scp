//! Copy one complete resource unchanged into an exclusive-created minimal overlay.
//! Usage: single_tgi_overlay <source.package> <out.package> <type> <group> <instance>
use std::{error::Error, fs::OpenOptions, io::Write};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: single_tgi_overlay <source> <output> <type> <group> <instance>".into());
    }
    let hex = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16);
    let id = dbpf::ResourceId {
        type_id: hex(&args[2])?,
        group: hex(&args[3])?,
        instance: hex(&args[4])?,
    };
    let source = dbpf::Package::open(&args[0])?;
    let entry = source
        .entries()
        .iter()
        .find(|e| e.id == id)
        .ok_or("TGI not found")?;
    let payload = source.read(entry)?;
    let bytes = dbpf::write_uncompressed_overlay(&[dbpf::OverlayEntry::new(id, payload.clone())])?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    output.write_all(&bytes)?;
    output.sync_all()?;
    drop(output);
    let check = dbpf::Package::open(&args[1])?;
    assert_eq!(check.entries().len(), 1);
    assert_eq!(check.entries()[0].id, id);
    assert_eq!(check.read(&check.entries()[0])?, payload);
    println!(
        "TGI={id} payload={} package={} verified=true",
        payload.len(),
        bytes.len()
    );
    Ok(())
}
