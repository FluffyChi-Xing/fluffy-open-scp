//! 列出 NH 包全部条目 TGI（离线诊断）。
use dbpf::Package;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    for e in pkg.entries() {
        println!(
            "{:08X} {:08X} {:08X} {}B",
            e.id.type_id,
            e.id.group,
            e.id.instance,
            pkg.read(e)?.len()
        );
    }
    Ok(())
}
