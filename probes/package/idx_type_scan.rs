//! 探针：跨包扫描索引中指定 type 的条目。仅开发用。
//! 用法: idx_type_scan <package> <0xTYPE,...>
fn main() -> dbpf::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("package");
    let types: Vec<u32> = args
        .next()
        .expect("types")
        .split(',')
        .map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let p = dbpf::Package::open(&path)?;
    for e in p.entries() {
        if types.contains(&e.id.type_id) {
            println!(
                "{:08X}:{:08X}:{:08X}  csize={} dsize={}",
                e.id.type_id, e.id.group, e.id.instance, e.compressed_size, e.decompressed_size
            );
        }
    }
    Ok(())
}
