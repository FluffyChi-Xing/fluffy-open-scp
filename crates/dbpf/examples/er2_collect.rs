//! 探针：收集目录下所有脚本包中的 0x08068AEC/AEB 条目并导出。仅开发用。
//! 用法: er2_collect <dir> <outdir>
use std::path::Path;

fn main() -> dbpf::Result<()> {
    let dir = std::env::args().nth(1).expect("dir");
    let outdir = std::env::args().nth(2).expect("outdir");
    std::fs::create_dir_all(&outdir).unwrap();
    let mut index = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().map(|e| e != "package").unwrap_or(true) {
            continue;
        }
        let package = dbpf::Package::open(&p)?;
        for e in package.entries() {
            if e.id.type_id == 0x08068AEC || e.id.type_id == 0x08068AEB {
                let data = package.read(e)?;
                let name = format!(
                    "{}_{}_{}_{}_{:08X}.{}",
                    p.file_name().unwrap().to_string_lossy().split('-').next().unwrap(),
                    if e.id.type_id == 0x08068AEB { "AEB" } else { "AEC" },
                    e.id.group,
                    e.id.instance,
                    e.decompressed_size,
                    if e.id.type_id == 0x08068AEB { "bin" } else { "txt" },
                );
                std::fs::write(Path::new(&outdir).join(&name), &data).unwrap();
                index.entry((e.id.type_id, e.id.group)).or_insert(Vec::new()).push(name);
            }
        }
    }
    for ((t, g), mut names) in index {
        names.sort();
        println!("{t:08X}:{g:08X} count={} ex={:?}", names.len(), names.first());
    }
    Ok(())
}
