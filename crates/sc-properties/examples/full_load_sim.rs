//! 模拟加载：包内全部条目按类型解析（model/texture/property/ERZ），
//! 暴露条目级损坏。
use dbpf::Package;
use rw4::Rw4File;
use sc_properties::PropertyFile;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).expect("package path");
    let pkg = Package::open(&path)?;
    let mut ok = 0usize;
    let mut skip = 0usize;
    let mut fail = Vec::new();
    for e in pkg.entries() {
        let data = pkg.read(e)?;
        let r = match e.id.type_id {
            0x2F4E_681B | 0x2F4E_681C => Rw4File::parse(&data)
                .map(|f| format!("{} sections", f.sections().len()))
                .map_err(|e| e.to_string()),
            0x00B1_B104 | 0x0A98_EAF0 | 0x00B2_CCCB => PropertyFile::parse(&data)
                .map(|p| format!("{} props", p.values.len()))
                .map_err(|e| e.to_string()),
            0x2F7D_0004 | 0x3F86_62EA => Ok("raw".to_string()),
            _ => {
                skip += 1;
                continue;
            }
        };
        match r {
            Ok(info) => ok += 1,
            Err(err) => fail.push(format!(
                "{:08X}/{:08X}/{:08X} ({}B): {}",
                e.id.type_id,
                e.id.group,
                e.id.instance,
                data.len(),
                err
            )),
        }
    }
    println!("{path}: {ok} parsed, {skip} skipped, {} FAILED", fail.len());
    for f in &fail {
        println!("  FAIL {f}");
    }
    Ok(())
}
