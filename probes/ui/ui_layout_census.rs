//! 探针：普查全部布局模块（type 0x67771F5C, group 0x0B074E5A），打印
//! instance / 大小 / 根 comment / 子节点数，并导出到 <out-dir>/<instance>.json。

use std::path::PathBuf;

fn main() -> dbpf::Result<()> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = PathBuf::from(args.remove(0));
    std::fs::create_dir_all(&out_dir).expect("create out dir");
    for path in &args {
        let Ok(package) = dbpf::Package::open(path) else { continue };
        let package_name = path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(path)
            .to_string();
        for entry in package.entries() {
            if entry.id.type_id != 0x6777_1F5C {
                continue;
            }
            let data = package.read(entry)?;
            let text = String::from_utf8_lossy(&data);
            let comment = text
                .split("\"comment\"")
                .nth(1)
                .and_then(|rest| rest.split('"').nth(1))
                .unwrap_or("")
                .to_string();
            println!(
                "{package_name}: {:08X}  {:>7}B  {}",
                entry.id.instance,
                data.len(),
                comment
            );
            std::fs::write(out_dir.join(format!("{:08X}.json", entry.id.instance)), &data)?;
        }
    }
    Ok(())
}
