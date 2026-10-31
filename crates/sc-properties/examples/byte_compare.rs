//! 一次性对拍：原版 property 原始字节 vs parse→encode_canonical 回写字节。
//! 用法：cargo run -p sc-properties --example byte_compare -- <pkg> <group_hex> <instance_hex>
use dbpf::Package;
use sc_properties::PropertyFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (pkg_path, group, instance) = (
        &args[1],
        u32::from_str_radix(args[2].trim_start_matches("0x"), 16)?,
        u32::from_str_radix(args[3].trim_start_matches("0x"), 16)?,
    );
    let pkg = Package::open(pkg_path)?;
    let entry = pkg
        .entries()
        .iter()
        .find(|e| e.id.group == group && e.id.instance == instance)
        .expect("entry not found");
    let original = pkg.read(entry)?;
    let file = PropertyFile::parse(&original)?;
    let reencoded = file.encode_canonical()?;
    println!(
        "original {} bytes, re-encoded {} bytes",
        original.len(),
        reencoded.len()
    );
    if original == reencoded {
        println!("BYTE-IDENTICAL ✓");
        return Ok(());
    }
    let n = original.len().min(reencoded.len());
    let mut diffs = Vec::new();
    for i in 0..n {
        if original[i] != reencoded[i] {
            diffs.push(i);
        }
    }
    println!(
        "{} differing bytes in common range (plus length delta {})",
        diffs.len(),
        reencoded.len() as i64 - original.len() as i64
    );
    let mut shown = 0;
    let mut i = 0;
    while i < n && shown < 12 {
        if original[i] != reencoded[i] {
            let start = i.saturating_sub(8);
            let end = (i + 8).min(n);
            println!(
                "  @{:#06x}: orig={:02x?} reenc={:02x?}",
                i,
                &original[start..end],
                &reencoded[start..end]
            );
            shown += 1;
            i += 8;
        } else {
            i += 1;
        }
    }
    Ok(())
}
