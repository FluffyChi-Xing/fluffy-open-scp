//! 探针：扫描一批包，统计每个包内 0x0A98EAF0 资源的数量与表 id 冲突情况。
//! 用于验证：原版是否只在 Locale 目录放置 locale 表；DLC 是否部分覆盖同名表。
//!
//! 用法：cargo run -p probes --release --example locale_mount_scan -- <package>...
use dbpf::Package;
use sc_properties::locale::LOCALE_RESOURCE_TYPE;
use std::collections::BTreeMap;

fn main() {
    let mut global: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for path in std::env::args().skip(1) {
        let Ok(p) = Package::open(&path) else {
            println!("SKIP (open fail): {path}");
            continue;
        };
        let mut ids = Vec::new();
        for e in p.entries() {
            if e.id.type_id == LOCALE_RESOURCE_TYPE {
                ids.push(e.id.instance);
                global.entry(e.id.instance).or_default().push(
                    path.rsplit(['\\', '/']).next().unwrap().to_string(),
                );
            }
        }
        let name = path.rsplit(['\\', '/']).next().unwrap();
        println!("{name}: {} locale tables{}", ids.len(), {
            if ids.is_empty() {
                String::new()
            } else {
                format!("，样本 {:?}",
                    ids.iter().take(6).map(|i| format!("{i:08X}")).collect::<Vec<_>>())
            }
        });
    }
    println!("---- 同一表 id 出现在多个包的情况 ----");
    let mut collisions = 0;
    for (id, pkgs) in &global {
        if pkgs.len() > 1 {
            collisions += 1;
            if collisions <= 20 {
                println!("  {id:08X}: {:?}", pkgs);
            }
        }
    }
    println!("共 {collisions} 个表 id 跨包重复");
}
