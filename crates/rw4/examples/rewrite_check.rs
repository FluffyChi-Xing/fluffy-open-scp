use dbpf::{OverlayEntry, Package, write_uncompressed_overlay_to_path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    let mut entries = Vec::new();
    let mut total = 0usize;
    for e in pkg.entries() {
        let data = pkg.read(e)?;
        total += data.len();
        entries.push(dbpf::OverlayEntry::new(e.id, data));
    }
    write_uncompressed_overlay_to_path(&args[1], &entries)?;
    let out = std::fs::metadata(&args[1])?.len();
    println!(
        "input payloads total {total} bytes, output file {} bytes, entries {}",
        total,
        entries.len()
    );
    Ok(())
}
