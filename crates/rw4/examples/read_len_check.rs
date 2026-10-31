use dbpf::Package;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    for e in pkg.entries() {
        if e.id.type_id == 0x2F4E_681B {
            let data = pkg.read(e)?;
            println!(
                "{:08X}:{:08X}:{:08X} decompressed_size={} read_len={} compressed={}",
                e.id.type_id,
                e.id.group,
                e.id.instance,
                e.decompressed_size,
                data.len(),
                e.compressed
            );
        }
    }
    Ok(())
}
