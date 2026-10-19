//! 反向搜索：哪个属性文件的哪个 hash 里出现了指定 Key instance。仅开发用。
use sc_properties::PropertyFile;

fn main() {
    let mut args = std::env::args().skip(1);
    let targets: Vec<u32> = args
        .next()
        .unwrap()
        .split(',')
        .map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap())
        .collect();
    for path in args {
        let Ok(package) = dbpf::Package::open(&path) else { continue };
        for entry in package.entries() {
            if entry.id.type_id != 0x00B1_B104 {
                continue;
            }
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else { continue };
            for prop in &file.values {
                let mut hits = Vec::new();
                for key in prop.keys() {
                    if targets.contains(&key.instance) {
                        hits.push(format!("key(0x{:08X})", key.instance));
                    }
                }
                if !hits.is_empty() {
                    println!(
                        "{} 0x{:08X} hash=0x{:08X} {} -> {}",
                        path.split('/').next_back().unwrap_or(path.as_str()),
                        entry.id.instance,
                        prop.hash,
                        prop.prop_type.name(),
                        hits.join(",")
                    );
                }
            }
        }
    }
}
