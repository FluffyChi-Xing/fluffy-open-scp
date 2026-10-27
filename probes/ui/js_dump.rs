//! 探针：转储 Game 包主 JS 脚本（type 67771F5C）供检索。仅开发用。
use std::path::Path;

fn main() {
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_Game.package").unwrap();
    let mut n = 0usize;
    for e in p.entries() {
        if e.id.type_id != 0x6777_1F5C {
            continue;
        }
        n += 1;
        let d = p.read(e).unwrap_or_default();
        let out = format!("tmp/js/{:08X}_{}.js", e.id.group, e.id.instance);
        std::fs::create_dir_all("tmp/js").unwrap();
        std::fs::write(Path::new(&out), &d).ok();
        println!("TGI {:08X}:{:08X}:{:08X} {} B -> {}", e.id.type_id, e.id.group, e.id.instance, d.len(), out);
    }
    println!("共 {n} 条 JS 资源");
}
