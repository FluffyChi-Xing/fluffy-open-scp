use openscp_sdk::{Result, package::Package};

fn main() -> Result<()> {
    let path = std::env::args_os().nth(1).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: inspect_package <file.package>",
        )
    })?;
    let package = Package::open(path)?;
    for entry in package.entries() {
        println!(
            "{} stored={} decoded={}",
            entry.id,
            entry.stored_len(),
            entry.decompressed_size
        );
    }
    Ok(())
}
