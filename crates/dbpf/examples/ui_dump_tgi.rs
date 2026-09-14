//! 探针：按完整 TGI 导出一个条目。用法: ui_dump_tgi <out> <type> <group> <instance> <package>

fn main() -> dbpf::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = std::path::PathBuf::from(&args[0]);
    let p = |s: &String| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
    let (t, g, i) = (p(&args[1]), p(&args[2]), p(&args[3]));
    let package = dbpf::Package::open(&args[4])?;
    for entry in package.entries() {
        if (entry.id.type_id, entry.id.group, entry.id.instance) == (t, g, i) {
            let data = package.read(entry)?;
            std::fs::write(&out, &data)?;
            println!("dumped {} bytes -> {}", data.len(), out.display());
            return Ok(());
        }
    }
    println!("not found");
    Ok(())
}
