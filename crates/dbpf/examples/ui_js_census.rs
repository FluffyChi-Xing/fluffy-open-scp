//! 探针 v2：导出 group 0x40464200 全部 JS 模块 + 松散关键词统计。

fn main() -> dbpf::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let package = dbpf::Package::open(&args[0])?;
    let out_dir = std::path::PathBuf::from(&args[1]);
    std::fs::create_dir_all(&out_dir)?;
    for entry in package.entries() {
        if entry.id.group != 0x4046_4200 || entry.id.type_id != 0x6777_1F5C {
            continue;
        }
        let data = package.read(entry)?;
        std::fs::write(out_dir.join(format!("{:08X}.js", entry.id.instance)), &data)?;
        println!(
            "{:08X}  {:>7}B  vertical={} horizontal={} proportional={} pinType={}",
            entry.id.instance,
            data.len(),
            data.windows(8).any(|w| w == b"vertical "),
            data.windows(10).any(|w| w == b"horizontal"),
            data.windows(12).any(|w| w == b"proportional"),
            data.windows(7).any(|w| w == b"pinType"),
        );
    }
    Ok(())
}
