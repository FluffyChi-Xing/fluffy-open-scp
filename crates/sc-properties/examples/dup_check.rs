//! 检查包内 TGI 重复（DBPF 索引冲突会导致加载器异常）。
use dbpf::Package;
use std::collections::HashMap;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in [
        "tmp/nh_r12_minimal.package",
        "tmp/nh_r11_menu_tower.package",
    ] {
        let pkg = Package::open(path)?;
        let mut seen: HashMap<(u32, u32, u32), usize> = HashMap::new();
        let mut dups = Vec::new();
        for e in pkg.entries() {
            let key = (e.id.type_id, e.id.group, e.id.instance);
            let c = seen.entry(key).or_insert(0);
            *c += 1;
            if *c == 2 {
                dups.push(key);
            }
        }
        println!(
            "{path}: {} entries, {} duplicate TGIs",
            pkg.entries().len(),
            dups.len()
        );
        for (t, g, i) in &dups {
            println!("  DUP {:08X}/{:08X}/{:08X}", t, g, i);
        }
    }
    Ok(())
}
